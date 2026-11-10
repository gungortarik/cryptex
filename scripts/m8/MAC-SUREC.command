#!/bin/bash
# Synthetic raw Mach sender proof; no launchd service or production IPC change.
set -euo pipefail
umask 077
export PATH='/usr/bin:/bin:/usr/sbin:/sbin'
export PYTHONDONTWRITEBYTECODE=1
if [ "$(/usr/bin/uname -s)" != Darwin ] || [ "$#" -ne 0 ]; then
    echo 'Bu dosya Mac uzerinde secenek eklemeden calistirilmalidir.'
    exit 2
fi
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
        printf 'scope=raw_mach_message_instance_probe_only\noutcome=launcher_failed\nrunner_exit=%s\n' "$code" > "$work/launcher.txt"
    fi
    files=()
    for name in result.json launcher.txt; do
        if [ -f "$work/$name" ]; then files+=("$name"); fi
    done
    if (cd "$work" && /usr/bin/zip -q "$work/MAC-SUREC-SONUC.zip" "${files[@]}"); then
        echo 'MAC-SUREC-SONUC.zip hazir. Bu dosyayi sohbete ekle.'
        /usr/bin/open -R "$work/MAC-SUREC-SONUC.zip"
    else
        code=2
    fi
    exit "$code"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
if ! /usr/bin/xcode-select -p >/dev/null 2>&1 || [ ! -x /usr/bin/python3 ]; then exit 2; fi
echo 'Gercek mesaj gonderen surecin kimligi sentetik olarak sinaniyor.'
echo 'Keychain codesign izni sorarsa bu islem icin Allow sec; Always Allow secme.'
/usr/bin/python3 -I -B "$base/mac_message_identity.py" --package "$base" --work "$work"
