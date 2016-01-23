.PHONY: default build test doc  clean


default: build test

build:
	cargo build $(CARGO_FLAGS)

test: build
	cargo test $(CARGO_FLAGS)

doc: build
	cargo doc --no-deps $(CARGO_FLAGS)

clean:
	rm -r target

gh-pages:
	git clone --branch gh-pages git@github.com:dgrunwald/rust-cancellation.git gh-pages

.PHONY: gh-pages-doc
gh-pages-doc: doc | gh-pages
	cd gh-pages && git pull
	rm -r gh-pages/doc
	cp -r target/doc gh-pages/
	rm gh-pages/doc/.lock
	cd gh-pages && git add .
	cd gh-pages && git commit -m "Update documentation"

publish: default gh-pages-doc
	cargo publish
	cd gh-pages && git push

