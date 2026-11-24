#!/bin/bash
set -euo pipefail
src="$(cd "$(dirname "$0")" && pwd)"
name=Cryptex-M17-Mac-Ilk-Kurulum
out="$RUNNER_TEMP/$name"
mkdir -p "$out/Resources"
install -m 755 "$src/KUR.command" "$out/KUR.command"
cp "$RUNNER_TEMP/Application.zip" "$out/Resources/Application.zip"
archive="$(shasum -a 256 "$out/Resources/Application.zip" | cut -d' ' -f1)"
sed -e "s/@ARCHIVE@/$archive/g" -e "s/@REVISION@/$BUILD_REVISION/g" "$src/install.sh" > "$out/Resources/install.sh"
xcrun clang -fobjc-arc -Wno-deprecated-declarations -Wall -Wextra -Werror -arch arm64 \
  -mmacosx-version-min=26.0 -framework Foundation -framework Security \
  "$src/keychain.m" -o "$out/Resources/keychain"
codesign --force --sign - --identifier com.cryptex.m17.enrollment "$out/Resources/keychain"
(cd "$RUNNER_TEMP" && /usr/bin/zip -qr "$name.zip" "$name")
shasum -a 256 "$RUNNER_TEMP/$name.zip"
