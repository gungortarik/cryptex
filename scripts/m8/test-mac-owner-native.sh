#!/bin/sh
set -eu
binary=$(mktemp "${TMPDIR:-/tmp}/cx-m8-native-test.XXXXXXXX")
trap 'rm -f "$binary"' EXIT
xcrun clang -std=c11 -Wall -Wextra -Werror services/cryptexd/tests/mac_owner_native.c \
    -framework Security -framework CoreFoundation -lbsm -o "$binary"
"$binary"
