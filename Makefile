all: build

AKG := target/release/authrorized-keys-github
build: $(AKG)

$(AKG): Cargo.toml Cargo.lock src/main.rs
	cargo build --release

check: $(AKG)
	cargo fmt --all -- --check

format:
	cargo fmt --all

.PHONY: test build
test: $(AKG)
	docker build -t authorized-keys-github-test .
	docker run -e TERM=xterm --privileged -i authorized-keys-github-test
