#!/bin/bash
set -euo pipefail
umask 077
export PATH=/usr/bin:/bin:/usr/sbin:/sbin
[[ "$(id -un)" == cryptexm14 && "$(id -u)" != 0 && "$(uname -m)" == arm64 ]]
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
[[ ! -L "$HOME/Applications" && ! -L "$parent" && ! -L "$state" && ! -L "$app" ]]
mkdir -p -m 700 "$HOME/Applications"
tmp="$(mktemp -d "$HOME/Applications/.cryptex-m17.XXXXXXXX")"
phase=paket
cp "$kit/Application.zip" "$tmp/input.zip"
revision='@REVISION@'
archive='@ARCHIVE@'
[[ "$(sha "$tmp/input.zip")" == "$archive" ]]
cert="$state/owner.der"
phase=anahtar
if [[ ! -e "$cert" && ! -L "$cert" ]]; then
  [[ ! -e "$parent" ]]
  cat > "$tmp/cert.cnf" <<'CERT'
[req]
distinguished_name=dn
[dn]
[signer]
basicConstraints=critical,CA:FALSE
keyUsage=critical,digitalSignature
extendedKeyUsage=codeSigning
CERT
  key="$(/usr/bin/openssl genrsa 2048)"
  printf '%s\n' "$key" | /usr/bin/openssl req -new -x509 -key /dev/stdin -sha256 -days 7 \
    -config "$tmp/cert.cnf" -extensions signer -subj "/CN=Cryptex M17 Mac Trial $(/usr/bin/uuidgen)" \
    -out "$tmp/public.pem"
  /usr/bin/openssl x509 -in "$tmp/public.pem" -outform DER -out "$tmp/public.der"
  printf '%s\n' "$key" | "$kit/keychain" enroll "$tmp/public.der"
  unset key
  "$kit/keychain" publish "$tmp/public.der" "$cert"
fi
[[ -f "$cert" && ! -L "$cert" ]]
pin="$(sha "$cert")"
selector="$(/usr/bin/shasum -a 1 "$cert" | cut -d' ' -f1)"
verify() {
  local bundle="$1" image role leaf
  [[ ! -L "$bundle" && ! -L "$bundle/Contents" && ! -L "$bundle/Contents/MacOS" ]]
  [[ ! -L "$bundle/Contents/MacOS/cryptexd" && ! -L "$bundle/Contents/MacOS/cryptex-desktop" ]]
  [[ "$(/usr/bin/plutil -extract CFBundleIdentifier raw "$bundle/Contents/Info.plist")" == com.cryptex.desktop ]]
  [[ "$(/usr/bin/plutil -extract CFBundleExecutable raw "$bundle/Contents/Info.plist")" == cryptex-desktop ]]
  for role in daemon desktop; do
    if [[ "$role" == daemon ]]; then image="$bundle/Contents/MacOS/cryptexd"; else image="$bundle"; fi
    /usr/bin/codesign --verify --strict --all-architectures -R "=identifier \"com.cryptex.$role\"" "$image"
    leaf="$tmp/$role."
    rm -f "${leaf}0"
    /usr/bin/codesign --display --architecture arm64 "--extract-certificates=$leaf" "$image"
    [[ "$(sha "${leaf}0")" == "$pin" ]] || return 1
  done
  [[ "$(/usr/bin/plutil -extract source.commit raw "$bundle/Contents/Resources/build-provenance.json")" == "$revision" ]]
}
approval() {
  printf '%s\n' "$revision" "$archive" "$pin" "$(sha "$1/Contents/MacOS/cryptex-desktop")" "$(sha "$1/Contents/MacOS/cryptexd")"
}
phase=kurulum
if [[ ! -e "$parent" ]]; then
  mkdir "$tmp/slot"
  /usr/bin/ditto -x -k "$tmp/input.zip" "$tmp/slot"
  candidate="$tmp/slot/Cryptex.app"
  [[ "$(/usr/bin/plutil -extract source.commit raw "$candidate/Contents/Resources/build-provenance.json")" == "$revision" ]]
  for executable in cryptex-desktop cryptexd; do
    [[ "$(/usr/bin/lipo -archs "$candidate/Contents/MacOS/$executable")" == arm64 ]]
  done
  phase=imza
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
[[ -f "$parent/approval" && ! -L "$parent/approval" ]]
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
