#!/bin/bash
set -eu
umask 077
if [[ "$(/usr/bin/id -un)" != cryptexm14 || "$(/usr/bin/id -u)" == 0 ]]; then
  echo 'Mac uzerinde cryptexm14 hesabina girip KUR.command dosyasini tekrar acin.'
  exit 2
fi
if [[ "$(/usr/bin/uname -m)" != arm64 ]]; then
  echo 'Bu paket Apple Silicon Mac icindir.'
  exit 2
fi
cd -P -- "$(dirname -- "$0")"
state="$HOME/Library/Application Support/Cryptex-M17-Ilk-Kurulum"
[[ ! -L "$state" ]] || exit 2
/bin/mkdir -p -m 700 "$state"
exec ./Resources/keychain lock "$state/trial.lock" "$PWD/Resources/install.sh"
