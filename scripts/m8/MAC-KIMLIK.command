#!/bin/bash
# Read-only prerequisite collection; no code signing, installation or service.
set -euo pipefail
umask 077
export PATH='/usr/bin:/bin:/usr/sbin:/sbin'
export PYTHONDONTWRITEBYTECODE=1
if [ "$(/usr/bin/uname -s)" != Darwin ] || [ "$#" -ne 0 ]; then
    echo 'Bu dosya gercek Mac uzerinde, secenek eklemeden calistirilmalidir.'
    exit 2
fi
base="$(cd "$(dirname "$0")" && pwd -P)"
for parent in /private/tmp /private /; do
    if [ -e "$parent/.git" ] || [ -L "$parent/.git" ]; then
        echo 'Guvenli gecici klasor bulunamadi.'
        exit 2
    fi
done
work="$(/usr/bin/mktemp -d '/private/tmp/cryptex-mac-kimlik.XXXXXX')"
result="$work/MAC-KIMLIK-SONUC.zip"
finish() {
    code=$?
    trap - EXIT
    set +e
    if [ ! -f "$work/result.json" ]; then
        printf 'schema=1\nscope=mac_identity_preflight_only\noutcome=launcher_failed\nrunner_exit=%s\nnative_xpc_matrix=not_run\n' "$code" > "$work/launcher.txt"
    fi
    files=()
    for name in result.json launcher.txt; do
        if [ -f "$work/$name" ]; then files+=("$name"); fi
    done
    if (cd "$work" && /usr/bin/zip -q "$result" "${files[@]}"); then
        echo 'MAC-KIMLIK-SONUC.zip hazir. Bu dosyayi sohbete ekle.'
        if [ "${CRYPTEX_MAC_PREFLIGHT_HEADLESS:-0}" = 1 ]; then
            for name in "${files[@]}"; do /bin/cat "$work/$name"; done
        else
            /usr/bin/open -R "$result"
        fi
    else
        echo 'Sonuc ZIP olusturulamadi.'
        code=2
    fi
    exit "$code"
}
trap finish EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Refuse rather than invoking Apple's tool-installation stub on a fresh Mac.
if ! /usr/bin/xcode-select -p >/dev/null 2>&1 || [ ! -x /usr/bin/python3 ]; then
    printf 'schema=1\nscope=mac_identity_preflight_only\noutcome=existing_developer_tools_missing\nnative_xpc_matrix=not_run\n' > "$work/launcher.txt"
    exit 2
fi
echo 'Mac SDK ve mevcut imzalama kimliklerinin hazirligi kontrol ediliyor...'
/usr/bin/python3 -I -B "$base/mac_identity_preflight.py" --package "$base" --work "$work"
