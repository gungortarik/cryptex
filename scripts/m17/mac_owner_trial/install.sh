#!/bin/bash
set -euo pipefail
umask 077
export PATH=/usr/bin:/bin:/usr/sbin:/sbin
[[ "$(id -un)" == cryptexm14 && "$(id -u)" != 0 && "$(uname -m)" == arm64 ]] || exit 2
kit="$(cd -P -- "$(dirname -- "$0")" && pwd)"
state="$HOME/Library/Application Support/Cryptex-M17-Ilk-Kurulum"
parent="$HOME/Applications/Cryptex-M17-Ilk-Kurulum"
app="$parent/Cryptex.app"
report="$(dirname "$kit")/M17-IMZA-SONUC-Darwin.json"
phase=hazirlik
tmp=''
receipt=''
finish() {
  code=$?
  trap - EXIT
  set +e
  if [[ "$code" != 0 ]]; then
    receipt="$(mktemp "${report}.XXXXXXXX")" &&
      printf '{"passed":false,"phase":"%s","exit_code":%d}\n' "$phase" "$code" > "$receipt" &&
      mv -f "$receipt" "$report"
    echo "Durduruldu: $phase. Sonuc: $report"
  fi
  if [[ -n "$tmp" && "$tmp" == "$HOME/Applications"/.cryptex-m17.* && -d "$tmp" && ! -L "$tmp" ]]; then /bin/rm -rf "$tmp"; fi
  exit "$code"
}
trap finish EXIT
sha() { /usr/bin/shasum -a 256 "$1" | /usr/bin/cut -d' ' -f1; }
[[ ! -L "$HOME/Applications" && ! -L "$parent" && ! -L "$state" && ! -L "$app" ]] || exit 2
mkdir -p -m 700 "$HOME/Applications"
tmp="$(mktemp -d "$HOME/Applications/.cryptex-m17.XXXXXXXX")"
phase=paket
cp "$kit/Application.zip" "$tmp/input.zip"
revision='@REVISION@'
archive='@ARCHIVE@'
observed="$(sha "$tmp/input.zip")"
if [[ "$observed" != "$archive" ]]; then
  echo "Uygulama arsivi degismis: $observed" >&2
  exit 3
fi
cert="$state/owner.der"
phase=anahtar
if [[ ! -e "$cert" && ! -L "$cert" ]]; then
  [[ ! -e "$parent" ]] || exit 2
  /bin/bash "$kit/enroll.sh" "$tmp"
fi
[[ -f "$cert" && ! -L "$cert" ]] || exit 2
pin="$(sha "$cert")"
selector="$(/usr/bin/shasum -a 1 "$cert" | cut -d' ' -f1)"
verify() {
  local bundle="$1" image role leaf
  [[ ! -L "$bundle" && ! -L "$bundle/Contents" && ! -L "$bundle/Contents/MacOS" ]] || exit 2
  [[ ! -L "$bundle/Contents/MacOS/cryptexd" && ! -L "$bundle/Contents/MacOS/cryptex-desktop" ]] || exit 2
  [[ "$(/usr/bin/plutil -extract CFBundleIdentifier raw "$bundle/Contents/Info.plist")" == com.cryptex.desktop ]] || exit 2
  [[ "$(/usr/bin/plutil -extract CFBundleExecutable raw "$bundle/Contents/Info.plist")" == cryptex-desktop ]] || exit 2
  for role in daemon desktop; do
    if [[ "$role" == daemon ]]; then image="$bundle/Contents/MacOS/cryptexd"; else image="$bundle"; fi
    /usr/bin/codesign --verify --strict --all-architectures -R "=identifier \"com.cryptex.$role\"" "$image"
    leaf="$tmp/$role."
    rm -f "${leaf}0"
    /usr/bin/codesign --display --architecture arm64 "--extract-certificates=$leaf" "$image"
    [[ "$(sha "${leaf}0")" == "$pin" ]] || return 1
  done
  [[ "$(/usr/bin/plutil -extract source.commit raw "$bundle/Contents/Resources/build-provenance.json")" == "$revision" ]] || exit 2
}
approval() {
  printf '%s\n' "$revision" "$archive" "$pin" "$(sha "$1/Contents/MacOS/cryptex-desktop")" "$(sha "$1/Contents/MacOS/cryptexd")"
}
phase=kurulum
if [[ ! -e "$parent" ]]; then
  mkdir "$tmp/slot"
  /usr/bin/ditto -x -k "$tmp/input.zip" "$tmp/slot"
  candidate="$tmp/slot/Cryptex.app"
  [[ "$(/usr/bin/plutil -extract source.commit raw "$candidate/Contents/Resources/build-provenance.json")" == "$revision" ]] || exit 2
  for executable in cryptex-desktop cryptexd; do
    [[ "$(/usr/bin/od -An -tx1 -N8 "$candidate/Contents/MacOS/$executable" | tr -d "[:space:]")" == cffaedfe0c000001 ]] || exit 2
  done
  phase=imza
  echo "Mac Anahtar Zinciri izin isterse cryptexm14 hesabinizla imzalamaya izin verin."
  /usr/bin/codesign --force --timestamp=none --options runtime --sign "$selector" \
    --identifier com.cryptex.daemon "$candidate/Contents/MacOS/cryptexd"
  /usr/bin/codesign --force --timestamp=none --options runtime --sign "$selector" \
    --identifier com.cryptex.desktop "$candidate"
  verify "$candidate"
  approval "$candidate" > "$tmp/slot/approval"
  "$kit/keychain" publish "$tmp/slot" "$parent"
fi
phase=dogrulama
verify "$app"
[[ -f "$parent/approval" && ! -L "$parent/approval" ]] || exit 2
approval "$app" > "$tmp/expected"
/usr/bin/cmp "$tmp/expected" "$parent/approval"
desktop="$(sha "$app/Contents/MacOS/cryptex-desktop")"
daemon="$(sha "$app/Contents/MacOS/cryptexd")"
receipt="$(mktemp "${report}.XXXXXXXX")"
printf '{"passed":true,"revision":"%s","input_archive_sha256":"%s","certificate_sha256":"%s","installed_desktop_sha256":"%s","installed_daemon_sha256":"%s"}\n' \
  "$revision" "$archive" "$pin" "$desktop" "$daemon" > "$receipt"
mv -f "$receipt" "$report"
echo "TAMAM. Kurulu masaustu ve servis imzalari dogrulandi."
echo "Sonuc: $report"
