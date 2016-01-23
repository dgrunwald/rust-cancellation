// Copyright (c) 2016 Daniel Grunwald
//
// Permission is hereby granted, free of charge, to any person obtaining a copy of this
// software and associated documentation files (the "Software"), to deal in the Software
// without restriction, including without limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons
// to whom the Software is furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all copies or
// substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED,
// INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR
// PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE
// FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

/*!
Rust-Cancellation is a small crate that provides the `CancellationToken` type
that can be used to signal cancellation to other code in a composable manner.

Operations that support cancellation usually accept a `ct: &CancellationToken` parameter.
They can either cooperatively check `ct.is_canceled()`, or use `ct.run()` to get notified
via callback when cancellation is requested.

Operations that finish asynchronously may instead accept `ct: Arc<CancellationToken>`.

To create a `CancellationToken`, use the type `CancellationTokenSource`.
A `CancellationTokenSource` contains a `Arc<CancellationToken>`
(which can be obtained using the `token()` method, or using deref coercions),
and additionally provides the `cancel()` operation to mark the token as canceled.

```rust
extern crate cancellation;
use cancellation::{CancellationToken, CancellationTokenSource, OperationCanceled};
use std::{time, thread};

fn cancelable_sum(values: &[i32], ct: &CancellationToken) -> Result<i32, OperationCanceled> {
    let mut sum = 0;
    for val in values {
        try!(ct.result());
        sum = sum + val;
        thread::sleep(time::Duration::from_secs(1));
    }
    Ok(sum)
}

fn main() {
    let cts = CancellationTokenSource::new();
    cts.cancel_after(time::Duration::from_millis(1500));
    assert_eq!(Err(OperationCanceled), cancelable_sum(&[1,2,3], &cts));
}
```

Using the `CancellationToken::run()` method, an action can be executed when the token is canceled.


```rust
extern crate cancellation;
use cancellation::{CancellationToken, CancellationTokenSource, OperationCanceled};
use std::{time, thread};
use std::time::Duration;

fn cancelable_sleep(dur: Duration, ct: &CancellationToken) -> Result<(), OperationCanceled> {
    let th = thread::current();
    ct.run(
        || { // the on_cancel closure runs on the canceling thread when the token is canceled
            th.unpark();
        },
        || { // this code block runs on the current thread and contains the cancelable operation
            thread::park_timeout(dur) // (TODO: handle spurious wakeups)
        }
    );
    if ct.is_canceled() {
        // The run() call above has a race condition: the on_cancel callback might call unpark()
        // after park_timeout gave up after waiting dur, but before the end of the run() call
        // deregistered the on_cancel callback.
        // We use a park() call with 0s timeout to consume the left-over parking token, if any.
        thread::park_timeout(Duration::from_secs(0));
        Err(OperationCanceled)
    } else {
        Ok(())
    }
}

fn main() {
    let cts = CancellationTokenSource::new();
    cts.cancel_after(Duration::from_millis(250));
    assert_eq!(Err(OperationCanceled), cancelable_sleep(Duration::from_secs(10), &cts));
}
```

**/

use std::{fmt, ops, mem, ptr, io, error, time, thread};
use std::sync::atomic::{AtomicUsize, ATOMIC_USIZE_INIT, Ordering};
use std::sync::{Arc, Mutex};

#[repr(C)]
pub struct CancellationTokenSource {
    token: Arc<CancellationToken>
}

const STATUS_CANNOT_BE_CANCELED : usize = 0;
const STATUS_NOT_CANCELED : usize = 1;
const STATUS_CANCELING : usize = 2;
const STATUS_CANCELED : usize = 3;

/// A CancellationToken is used to query whether an operation should be canceled.
///
/// To cancel a token, use `CancellationTokenSource`.
///
/// Use `CancellationTokenSource` to obtain instances of `CancellationToken`.
pub struct CancellationToken {
    status: AtomicUsize,
    // Use mutex so we don't have to worry about thread-safety while managing the registrations.
    // The mutex also ensures that `CancellationToken::run()` can't return while the on_cancel callback is still running.
    // The option around the mutex allows us to construct the NO_CANCELLATION token.
    
    // The `*mut Registration` points to the first active registration.
    // Registrations are connected in a double-linked-list in order to
    // support O(1) removal.
    // The back-link (`Registration::link_to_this`) is of type `*mut *mut Registration`
    // and may refer to the contents of this mutex (for the first node) or the `Registration::next` of the previous node.
    registrations: Option<Mutex<*mut Registration<'static>>>
}

// AtomicUsize and Mutex are both Sync;
// we need this unsafe impl only because *mut isn't Send.
unsafe impl Sync for CancellationToken {}
unsafe impl Send for CancellationToken {}

static NO_CANCELLATION: CancellationToken = CancellationToken {
    status: ATOMIC_USIZE_INIT, //AtomicUsize::new(STATUS_CANNOT_BE_CANCELED),
    registrations: None
};

/// Unit struct used to indicate that an operation was canceled.
///
/// Usually used as `Result<T, OperationCanceled>`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperationCanceled;

// Helper trait for Option<C> where C:FnOnce()
trait FnOnceOption {
    // equivalent to self.take().map(|c| c())
    fn call_once(&mut self) -> Option<()>;
}

impl<C> FnOnceOption for Option<C> where C: FnOnce() {
    fn call_once(&mut self) -> Option<()> {
        self.take().map(|c| c())
    }
}

/// Registrations are the entries in the linked list of on_cancel callbacks.
/// They are unsafely shared across threads.
/// Access is synchronized using the cancellation token's mutex.
struct Registration<'a> {
    on_cancel: &'a mut (FnOnceOption + Send + 'a),
    cancellation_token: &'a CancellationToken,
    // Next registration in the linked list.
    next: *mut Registration<'static>,
    // Link to the previous node's next field.
    // For the first node, this points to the contents of the CancellationToken::registration mutex.
    // The address of the pointed-to-field is stable:
    // Registrations are never moved (they only exist in `CancellationToken::run()`'s stack frame);
    // and we know the CancellationToken cannot move because it's being borrowed by this registration.
    link_to_this: *mut *mut Registration<'static>
}

unsafe fn erase_lifetime(r: &mut Registration) -> *mut Registration<'static> {
    mem::transmute(r)
}

/// Remove registration from the linked list of registrations.
/// May only be called while the registration mutex is acquired.
/// Assumes that r.link_to_this is not null.
unsafe fn unlink(r: &mut Registration) {
    assert!(*r.link_to_this == erase_lifetime(r));
    // let previous node point to the next:
    *r.link_to_this = r.next;
    if !r.next.is_null() {
        // update link_to_this of the next node
        (*r.next).link_to_this = r.link_to_this
    }
    // null out the links of the removed node
    r.link_to_this = ptr::null_mut();
    r.next = ptr::null_mut();
}

impl CancellationTokenSource {
    pub fn new() -> CancellationTokenSource {
        CancellationTokenSource {
            token: Arc::new(CancellationToken {
                status: AtomicUsize::new(STATUS_NOT_CANCELED),
                registrations: Some(Mutex::new(ptr::null_mut()))
            })
        }
    }

    /// Gets the token managed by this CancellationTokenSource.
    ///
    /// The `Arc` can be cloned so that the token can be passed into
    /// asynchronous tasks that might outlive the CancellationTokenSource.
    #[inline]
    pub fn token(&self) -> &Arc<CancellationToken> {
        &self.token
    }

    /// Marks the cancellation token as canceled.
    ///
    /// Executes the `on_cancel` callback of any active `CancellationToken::run` invocations.
    /// Has no effect if the cancellation token was already canceled.
    pub fn cancel(&self) {
        self.token.cancel()
    }

    /// Creates a new, detached thread that waits for the specified duration
    /// and then marks the cancellation token as canceled.
    pub fn cancel_after(&self, dur: time::Duration) {
        let token = self.token.clone();
        thread::spawn(move || {
            thread::sleep(dur);
            token.cancel()
        });
    }
}

impl CancellationToken {
    /// Returns a reference to a cancellation token that is never canceled.
    #[inline]
    pub fn none() -> &'static CancellationToken {
        &NO_CANCELLATION
    }

    fn status_string(&self) -> &'static str {
        match self.status.load(Ordering::Acquire) {
            STATUS_CANNOT_BE_CANCELED => "cannot be canceled",
            STATUS_NOT_CANCELED => "not canceled",
            STATUS_CANCELING => "canceling",
            STATUS_CANCELED => "canceled",
            _ => "invalid"
        }
    }

    /// Gets whether this token has been canceled.
    ///
    /// This function is inherently racy: it may start returning `true`
    /// at any moment as the token is canceled by another thread.
    ///
    /// However, once the function returns `true`, it will always keep returning `true`
    /// (cancellation tokens cannot be reset).
    #[inline]
    pub fn is_canceled(&self) -> bool {
        self.status.load(Ordering::Acquire) >= STATUS_CANCELING
    }

    /// Returns `Ok(())` if this token has not been canceled.
    /// Returns `Err(OperationCanceled)` if this token has been canceled.
    ///
    /// This is an alternative to `is_canceled()` that can be
    /// used with the `try!()` macro.
    #[inline]
    pub fn result(&self) -> Result<(), OperationCanceled> {
        if self.is_canceled() {
            Err(OperationCanceled)
        } else {
            Ok(())
        }
    }

    fn cancel(&self) {
        if self.is_canceled() {
            // avoid deadlock if cancel() is called within on_cancel callback
            return;
        }
        let mut registrations = self.registrations.as_ref().unwrap().lock().unwrap();
        let status = self.status.load(Ordering::Relaxed);
        if status == STATUS_CANCELED {
            return; // already canceled
        }
        assert!(status == STATUS_NOT_CANCELED);
        self.status.store(STATUS_CANCELING, Ordering::Release);
        while !registrations.is_null() {
            unsafe {
                let registration = &mut **registrations;
                unlink(registration);
                registration.on_cancel.call_once();
            }
        }
        self.status.store(STATUS_CANCELED, Ordering::Release);
    }

    /// Runs function `f` on the current thread.
    /// If the token is canceled while `f` is running,
    /// the `on_cancel` function will be executed by the
    /// thread calling `cancel()`.
    ///
    /// If the `f` callback returns before the `on_cancel` callback does,
    /// `run()` waits for the `on_cancel` callback to finish before returning.
    /// Any memory writes performed by the `on_cancel` callback will be visible
    /// after `run()` returns.
    ///
    /// If the token is already canceled when this function
    /// is called, `on_cancel` will be executed on the
    /// current thread before `f` is called.
    ///
    /// If the token is not canceled during the execution of `f`, the
    /// `on_cancel` callback will not be called at all.
    ///
    /// Panics in the `f` callback are supported: unwinding will wait for
    /// `on_cancel` to finish (if it is running concurrently) and unregister
    /// the `on_cancel` callback from the token.
    ///
    /// Panics in the `on_cancel` callback may abort the process.
    pub fn run<C, F, R>(&self, on_cancel: C, f: F) -> R
        where C: FnOnce() + Send,
              F: FnOnce() -> R
    {
        let mut on_cancel = Some(on_cancel);
        // Create a dummy registration
        let mut registration: Option<Registration> = None;

        // Initialization part is extracted into new function so that it doesn't get
        // unnecessarily monomorphized.
        fn init_registration<'a>(
                token: &'a CancellationToken,
                on_cancel: &'a mut (FnOnceOption + Send + 'a),
                registration: &mut Option<Registration<'a>>)
        {
            // Check the status before acquiring the lock.
            // This is important to avoid deadlocks when the token is re-used within an on_cancel callback.
            match token.status.load(Ordering::Acquire) {
                STATUS_CANNOT_BE_CANCELED => { }
                STATUS_NOT_CANCELED => {
                    let mut mutex_guard = token.registrations.as_ref().unwrap().lock().unwrap();
                    // the status might have changed while we waited for the lock
                    match token.status.load(Ordering::Relaxed) {
                        STATUS_NOT_CANCELED => {
                            // Insert registration into linked list
                            let first_registration: &mut *mut Registration = &mut *mutex_guard;
                            *registration = Some(Registration {
                                on_cancel: on_cancel,
                                cancellation_token: token,
                                next: *first_registration,
                                link_to_this: first_registration
                            });
                            // Erasing the lifetime of the registration is safe,
                            // because the Drop impl of the registration will undo this assignment
                            // before on_cancel is dropped.
                            *first_registration = unsafe { erase_lifetime(registration.as_mut().unwrap()) };
                        },
                        STATUS_CANCELED => {
                            // if already canceled, run the on_cancel callback immediately
                            on_cancel.call_once();
                        },
                        _ => {
                            // STATUS_CANNOT_BE_CANCELED is handled by the outer `match`,
                            // STATUS_CANCELING should be impossible to observe within the mutex
                            panic!("invalid status")
                        }
                    }
                }, // release mutex
                STATUS_CANCELING | STATUS_CANCELED => {
                    // If already canceling/canceled, run the on_cancel callback immediately
                    // It's important to handle this case in the outer match
                    on_cancel.call_once();
                },
                _ => {
                    panic!("invalid status")
                }
            }
        }
        init_registration(self, &mut on_cancel, &mut registration);
        return f();

        // The registration will be dropped automatically here
        impl <'a> Drop for Registration<'a> {
            fn drop(&mut self) {
                let _mutex_guard = self.cancellation_token.registrations.as_ref().unwrap().lock().unwrap();
                if !self.link_to_this.is_null() {
                    unsafe { unlink(self); }
                }
            }
        }
    }
}

impl fmt::Debug for CancellationTokenSource {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("CancellationTokenSource")
            .field("status", &self.status_string())
            .finish()
    }
}

impl fmt::Debug for CancellationToken {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.debug_struct("CancellationToken")
            .field("status", &self.status_string())
            .finish()
    }
}

impl ops::Deref for CancellationTokenSource {
    type Target = CancellationToken;

    #[inline]
    fn deref(&self) -> &CancellationToken {
        &self.token
    }
}

impl fmt::Display for OperationCanceled {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        f.write_str(error::Error::description(self))
    }
}

impl error::Error for OperationCanceled {
    fn description(&self) -> &'static str {
        "The operation was canceled."
    }
}

impl From<OperationCanceled> for io::Error {
    fn from(oc: OperationCanceled) -> Self {
        io::Error::new(io::ErrorKind::TimedOut, oc)
    }
}

#[cfg(test)]
mod test {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use super::*;

    #[test]
    fn none_not_canceled() {
        assert_eq!(false, CancellationToken::none().is_canceled());
    }

    #[test]
    fn none_run() {
        let b = AtomicBool::new(false);
        assert_eq!(42, CancellationToken::none().run(
            || b.store(true, Ordering::Relaxed),
            || 42
        ));
        assert_eq!(false, b.load(Ordering::Relaxed));
    }

    #[test]
    fn cancel() {
        let cts = CancellationTokenSource::new();
        assert_eq!(false, cts.is_canceled());
        assert_eq!(Ok(()), cts.result());
        cts.cancel();
        assert_eq!(true, cts.is_canceled());
        assert_eq!(Err(OperationCanceled), cts.result());
    }

    fn expect(state: &AtomicUsize, expected_state: usize) {
        assert_eq!(state.load(Ordering::Acquire), expected_state);
        state.store(expected_state + 1, Ordering::Release);
    }

    #[test]
    fn run_already_canceled() {
        let cts = CancellationTokenSource::new();
        cts.cancel();
        let state = AtomicUsize::new(0);
        expect(&state, 0);
        cts.run(
            || {
                assert!(cts.is_canceled());
                expect(&state, 1);
            },
            || expect(&state, 2)
        );
        expect(&state, 3);
    }

    #[test]
    fn recursive_run_already_canceled() {
        let cts = CancellationTokenSource::new();
        cts.cancel();
        let state = AtomicUsize::new(0);
        cts.run(
            || cts.run(
                || expect(&state, 0),
                || expect(&state, 1),
               ),
            || cts.run(
                || expect(&state, 2),
                || expect(&state, 3),
        ));
        expect(&state, 4);
    }

    #[test]
    fn cancel_in_recursive_run() {
        let cts = CancellationTokenSource::new();
        let state = AtomicUsize::new(0);
        cts.run(
            || expect(&state, 3),
            || {
                expect(&state, 0);
                cts.run(
                    || expect(&state, 2),
                    || {
                        expect(&state, 1);
                        cts.cancel();
                        expect(&state, 4);
                       }
                );
                expect(&state, 5);
            }
        );
        expect(&state, 6);
    }

    #[test]
    fn on_cancel_is_not_called_after_end_of_run() {
        let cts = CancellationTokenSource::new();
        let state = AtomicUsize::new(0);
        cts.run(
            || expect(&state, 2),
            || {
                cts.run(
                    || panic!("bad!"),
                    || expect(&state, 0)
                );
                expect(&state, 1);
                cts.cancel();
                expect(&state, 3);
            });
    }
}
