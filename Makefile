all: build

AKG := target/release/authrorized-keys-github

# When building the testing image, make the UID and GID match ours
UID := $(shell id -u)
GID := $(shell id -g)

# We like to be fancy
COLS := $(shell tput cols 2>/dev/null)
ifeq ($(COLS),)
COLS := 80
endif

# If we're supposed to use `docker`, do so with the image in `$(1)`.
# Otherwise, just run natively.  When running in `docker`, we use `--privileged`
# to give the docker container the ability to `mount`.
ifeq ($(USE_DOCKER),true)
define docker_exec
docker run --privileged \
           --user=$(shell id -u):$(shell id -g) \
           -e TERM=xterm \
		   -e CARGO_HOME=/usr/local/cargo_home \
		   -e CARGO_TERM_PROGRESS_WIDTH=$(COLS) \
		   -e CARGO_TERM_PROGRESS_WHEN=always \
		   -v $(shell pwd)/target/.docker_cargo_home:/usr/local/cargo_home \
           -v $(shell pwd):/app \
		   -w /app \
		   -i \
		   $(1) \
		   $(2)
endef

# To speed up interactive development with cargo, cache the cargo home in a subdir of `target`
target/.docker_cargo_home:
	mkdir -p $@
$(AKG): target/.docker_cargo_home
else
define docker_exec
$(2)
endef
endif

$(AKG): Cargo.toml Cargo.lock src/main.rs
	$(call docker_exec,rust,cargo build --color=always --release)
build: $(AKG)

check:
	$(call docker_exec,rust,cargo fmt --color=always --all -- --check)

format:
	$(call docker_exec,rust,cargo fmt --color=always --all)

.PHONY: test build
test: $(NATIVE_EXE)
ifeq ($(USE_DOCKER),true)
	docker build --build-arg UID=$(UID) --build-arg GID=$(GID) -t authorized-keys-github-test .
endif
	$(call docker_exec,authorized-keys-github-test,bash runtests.sh)

clean:
	rm -rf target
