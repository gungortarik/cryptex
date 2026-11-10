#!/bin/bash
# Isolated candidate only. No product IPC, profiles or private keys are exported.
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
        printf 'scope=macos_atomic_launch_same_organization_candidate_only\noutcome=launcher_failed\nrunner_exit=%s\n' "$code" > "$work/launcher.txt"
    fi
    files=()
    for name in result.json launcher.txt; do
        if [ -f "$work/$name" ]; then files+=("$name"); fi
    done
    if (cd "$work" && /usr/bin/zip -q "$work/MAC-BAGLANTI-SONUC.zip" "${files[@]}"); then
        echo 'MAC-BAGLANTI-SONUC.zip hazir. Bu dosyayi sohbete ekle.'
        /usr/bin/open -R "$work/MAC-BAGLANTI-SONUC.zip"
    else
        code=2
    fi
    exit "$code"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
if ! /usr/bin/xcode-select -p >/dev/null 2>&1 || [ ! -x /usr/bin/python3 ]; then exit 2; fi
echo 'Baslangic kanalinin miras kalmamasi ve imzali paket kimligi sinaniyor.'
echo 'Keychain codesign izni sorarsa bu islem icin Allow sec; Always Allow secme.'
/usr/bin/python3 -I -B "$base/mac_launch_security.py" --package "$base" --work "$work"
