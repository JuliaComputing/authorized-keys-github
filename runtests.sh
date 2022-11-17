#!/bin/bash
set -euo pipefail

# If we're running the tests locally, we should build `authorized-keys-github` and add it to our `PATH`
# On CI, we'll be running in a docker container that will already contain this file.
if ! command -v authorized-keys-github >/dev/null; then
    cargo build --release
    export PATH=$PATH:$(pwd)/target/release
fi

# Always print rust backtraces
export RUST_BACKTRACE=full

header() {
    tput bold
    tput setaf 1
    echo "$*"
    tput sgr0
}

die() {
    echo "ERROR: $*" >&2
    exit 1
}

# Create keys directory as a tmpfs that we can flip to read-only at will
KEYS_DIR=$(mktemp -d)
header "Mounting tmpfs at ${KEYS_DIR}"
sudo mount -t tmpfs -o size=1M tmpfs ${KEYS_DIR}
trap "sudo umount ${KEYS_DIR}; rm -rf ${KEYS_DIR}" EXIT
echo

fill_up_disk_space() {
    dd if=/dev/zero of="${1}/space_filler" bs=1 count=$((1*1024*1024)) 2>/dev/null || true
}

test_output() {
    OUTPUT="${1}"
    EXPECTING_REFRESH="${2}"
    EXPECTING_KEYS="${3}"
    if [[ -n "${EXPECTING_REFRESH}" ]]; then
        if ! grep -q "Refreshing keys from github" <<<"${OUTPUT}"; then
            die "Missing GitHub refresh!"
        fi
    else
        if grep -q "Refreshing keys from github" <<<"${OUTPUT}"; then
            die "Unexpected github refresh!"
        fi
    fi

    if [[ -n "${EXPECTING_KEYS}" ]]; then
        if ! grep -q "ssh-rsa" <<<"${OUTPUT}"; then
            die "No keys found!"
        fi
    else
        if grep -q "ssh-rsa" <<<"${OUTPUT}"; then
            die "Unexpected github keys!"
        fi
    fi
}

# Basic test: Just see if it runs
header "Basic test, should print all github keys for current user:"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "REFRESH_EXPECTED" "KEYS_EXPECTED"

# Caching test; ensure that running it a second time doesn't hit GitHub
header "Caching test, should not hit github a second time"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "" "KEYS_EXPECTED"

# Fingerprint test; ensure that giving a real fingerprint works:
header "Fingerprint test, positive"
FINGERPRINT="$(ssh-keygen -lf ${KEYS_DIR}/$(id -u).keys | cut -d' ' -f 2)"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" --fp="${FINGERPRINT}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "" "KEYS_EXPECTED"

# Fingerprint test; ensure that giving a fake fingerprint doesn't work:
header "Fingerprint test, negative"
FINGERPRINT="SHA256:0000000000000000000000000000000000000000000"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" --fp=${FINGERPRINT} $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "REFRESH_EXPECTED" ""


# Fill up tmpfs to generate out-of-space errors:
fill_up_disk_space "${KEYS_DIR}"

# Disk space exhaustion test; we don't panic on a cache hit:
header "Disk space exhaustion test, positive:"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "" "KEYS_EXPECTED"

# Disk space exhaustion test; we don't panic on a cache miss:
header "Disk space exhaustion test, negative:"
FINGERPRINT="SHA256:0000000000000000000000000000000000000000000"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" --fp="${FINGERPRINT}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "REFRESH_EXPECTED" ""

# Test that when the disk is full, even with a positive result, but a cold cache, we still print out
# even though we can't cache the result, and that if we do it twice, we just continually refresh.
sudo umount "${KEYS_DIR}"
sudo mount -t tmpfs -o size=1M tmpfs ${KEYS_DIR}
fill_up_disk_space "${KEYS_DIR}"
header "Disk space exhaustion test, positive but uncacheable:"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "REFRESH_EXPECTED" "KEYS_EXPECTED"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "REFRESH_EXPECTED" "KEYS_EXPECTED"


# Next, do the above tests again, but this time with `${KEYS_DIR}` as read-only, to hit new
# error paths:
sudo umount "${KEYS_DIR}"
sudo mount -t tmpfs -o size=1M tmpfs ${KEYS_DIR}

# Run once to fill the cache
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "REFRESH_EXPECTED" "KEYS_EXPECTED"

# Remount as read-only
sudo mount -o remount,ro "${KEYS_DIR}"

# read-only test; we don't panic on a cache hit:
header "Read-only test, positive:"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "" "KEYS_EXPECTED"

# read-only test; we don't panic on a cache miss:
header "Read-only test, negative:"
FINGERPRINT="SHA256:0000000000000000000000000000000000000000000"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" --fp="${FINGERPRINT}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "REFRESH_EXPECTED" ""

# Test that when the disk is full, even with a positive result, but a cold cache, we still print out
# even though we can't cache the result, and that if we do it twice, we just continually refresh.
sudo umount "${KEYS_DIR}"
sudo mount -t tmpfs -o size=1M tmpfs ${KEYS_DIR}
fill_up_disk_space "${KEYS_DIR}"
header "Read-only test, positive but uncacheable:"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "REFRESH_EXPECTED" "KEYS_EXPECTED"
OUTPUT="$(authorized-keys-github --keys-dir="${KEYS_DIR}" $(id -u) 2>&1 | tee >(cat 1>&2))"
test_output "${OUTPUT}" "REFRESH_EXPECTED" "KEYS_EXPECTED"
