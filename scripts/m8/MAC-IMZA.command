#!/bin/bash
# Temporary synthetic XPC service and signed nonce-echo fixtures only.
set -euo pipefail
umask 077
export PATH='/usr/bin:/bin:/usr/sbin:/sbin'
export PYTHONDONTWRITEBYTECODE=1
if [ "$(/usr/bin/uname -s)" != Darwin ] || [ "$#" -ne 0 ]; then
    echo 'Bu dosya Mac uzerinde, secenek eklemeden calistirilmalidir.'
    exit 2
fi
base="$(cd "$(dirname "$0")" && pwd -P)"
for parent in /private/tmp /private /; do
    if [ -e "$parent/.git" ] || [ -L "$parent/.git" ]; then exit 2; fi
done
work="$(/usr/bin/mktemp -d '/private/tmp/cryptex-mac-imza.XXXXXX')"
result="$work/MAC-IMZA-SONUC.zip"
finish() {
    code=$?
    trap - EXIT
    set +e
    if [ -f "$work/job-owned.json" ]; then
        /usr/bin/python3 -I -B "$base/mac_xpc_probe.py" --package "$base" --work "$work" --cleanup
        if [ "$?" -ne 0 ]; then
            printf 'temporary_job_cleanup=unconfirmed\n' > "$work/cleanup.txt"
            echo 'Gecici servis temizligi dogrulanamadi; sonucu gonder.'
            code=2
        fi
    fi
    if [ ! -f "$work/result.json" ]; then
        printf 'schema=1\nscope=synthetic_signed_xpc_observations_only\noutcome=launcher_failed\nrunner_exit=%s\n' "$code" > "$work/launcher.txt"
    fi
    files=()
    for name in result.json launcher.txt cleanup.txt; do
        if [ -f "$work/$name" ]; then files+=("$name"); fi
    done
    if (cd "$work" && /usr/bin/zip -q "$result" "${files[@]}"); then
        echo 'MAC-IMZA-SONUC.zip hazir. Bu dosyayi sohbete ekle.'
        /usr/bin/open -R "$result"
    else
        code=2
    fi
    exit "$code"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
if ! /usr/bin/xcode-select -p >/dev/null 2>&1 || [ ! -x /usr/bin/python3 ]; then exit 2; fi
echo 'Sentetik XPC testi calisiyor. Keychain sorarsa yalnizca bu islem icin Izin Ver sec.'
echo 'Her zaman izin verme veya guvenlik ayarlarini degistirme. Iptal edebilirsin.'
/usr/bin/python3 -I -B "$base/mac_xpc_probe.py" --package "$base" --work "$work"
