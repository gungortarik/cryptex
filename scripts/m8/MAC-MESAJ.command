#!/bin/bash
# Signed fixture only; no product endpoint, profile or permanent service.
set -euo pipefail
umask 077
export PATH='/usr/bin:/bin:/usr/sbin:/sbin'
export PYTHONDONTWRITEBYTECODE=1
if [ "$(/usr/bin/uname -s)" != Darwin ] || [ "$#" -ne 0 ]; then exit 2; fi
base="$(cd "$(dirname "$0")" && pwd -P)"
for parent in /private/tmp /private /; do
    if [ -e "$parent/.git" ] || [ -L "$parent/.git" ]; then exit 2; fi
done
work="$(/usr/bin/mktemp -d '/private/tmp/cryptex-mac-imza.XXXXXX')"
finish() {
    code=$?
    trap - EXIT
    set +e
    if [ ! -f "$work/result.json" ]; then
        printf 'scope=macos_public_message_bootstrap_signed_two_binary_candidate_only\noutcome=launcher_failed\nrunner_exit=%s\n' "$code" > "$work/launcher.txt"
    fi
    files=()
    for name in result.json launcher.txt; do
        if [ -f "$work/$name" ]; then files+=("$name"); fi
    done
    if (cd "$work" && /usr/bin/zip -q "$work/MAC-MESAJ-SONUC.zip" "${files[@]}"); then
        echo 'MAC-MESAJ-SONUC.zip hazir. Bu dosyayi sohbete ekle.'
        /usr/bin/open -R "$work/MAC-MESAJ-SONUC.zip"
    else
        code=2
    fi
    exit "$code"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
if ! /usr/bin/xcode-select -p >/dev/null 2>&1 || [ ! -x /usr/bin/python3 ]; then exit 2; fi
echo 'Imzali Desktop/daemon icin public mesaj baslangici ve sahte surecler sinaniyor.'
echo 'Keychain codesign izni sorarsa bu islem icin Allow sec; Always Allow secme.'
/usr/bin/python3 -I -B "$base/mac_bootstrap_signed.py" --package "$base" --work "$work"
