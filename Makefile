all: build

AKG := target/release/authrorized-keys-github
build: $(AKG)

$(AKG): Cargo.toml Cargo.lock src/main.rs
	cargo build --release

.PHONY: test build
test: $(AKG)
	docker build -t authorized-keys-github-test .
	docker run --privileged -ti authorized-keys-github-test
