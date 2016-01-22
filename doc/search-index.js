var searchIndex = {};
searchIndex['cancellation'] = {"items":[[3,"CancellationTokenSource","cancellation","",null,null],[3,"CancellationToken","","A CancellationToken is used to query whether an operation should be canceled.",null,null],[3,"OperationCanceled","","Unit struct used to indicate that an operation was canceled.",null,null],[11,"eq","","",0,{"inputs":[{"name":"operationcanceled"},{"name":"operationcanceled"}],"output":{"name":"bool"}}],[11,"ne","","",0,{"inputs":[{"name":"operationcanceled"},{"name":"operationcanceled"}],"output":{"name":"bool"}}],[11,"fmt","","",0,{"inputs":[{"name":"operationcanceled"},{"name":"formatter"}],"output":{"name":"result"}}],[11,"clone","","",0,{"inputs":[{"name":"operationcanceled"}],"output":{"name":"operationcanceled"}}],[11,"new","","",1,{"inputs":[{"name":"cancellationtokensource"}],"output":{"name":"cancellationtokensource"}}],[11,"token","","Gets the token managed by this CancellationTokenSource.",1,{"inputs":[{"name":"cancellationtokensource"}],"output":{"name":"arc"}}],[11,"cancel","","Marks the cancellation token as canceled.",1,{"inputs":[{"name":"cancellationtokensource"}],"output":null}],[11,"cancel_after","","Creates a new, detached thread that waits for the specified duration\nand then marks the cancellation token as canceled.",1,{"inputs":[{"name":"cancellationtokensource"},{"name":"duration"}],"output":null}],[11,"none","","Returns a reference to a cancellation token that is never canceled.",2,{"inputs":[{"name":"cancellationtoken"}],"output":{"name":"cancellationtoken"}}],[11,"is_canceled","","Gets whether this token has been canceled.",2,{"inputs":[{"name":"cancellationtoken"}],"output":{"name":"bool"}}],[11,"result","","Returns `Ok(())` if this token has not been canceled.\nReturns `Err(OperationCanceled)` if this token has been canceled.",2,{"inputs":[{"name":"cancellationtoken"}],"output":{"name":"result"}}],[11,"run","","Runs function `f` on the current thread.\nIf the token is canceled while `f` is running,\nthe `on_cancel` function will be executed by the\nthread calling `cancel()`.",2,{"inputs":[{"name":"cancellationtoken"},{"name":"c"},{"name":"f"}],"output":{"name":"r"}}],[11,"fmt","","",1,{"inputs":[{"name":"cancellationtokensource"},{"name":"formatter"}],"output":{"name":"result"}}],[11,"fmt","","",2,{"inputs":[{"name":"cancellationtoken"},{"name":"formatter"}],"output":{"name":"result"}}],[11,"deref","","",1,{"inputs":[{"name":"cancellationtokensource"}],"output":{"name":"cancellationtoken"}}],[11,"fmt","","",0,{"inputs":[{"name":"operationcanceled"},{"name":"formatter"}],"output":{"name":"result"}}],[11,"description","","",0,{"inputs":[{"name":"operationcanceled"}],"output":{"name":"str"}}],[11,"from","std::io::error","",3,{"inputs":[{"name":"error"},{"name":"operationcanceled"}],"output":{"name":"self"}}]],"paths":[[3,"OperationCanceled"],[3,"CancellationTokenSource"],[3,"CancellationToken"],[3,"Error"]]};
initSearch(searchIndex);