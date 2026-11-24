#!/bin/bash
set -euo pipefail
[[ "$GITHUB_ACTIONS" == true && "$RUNNER_ENVIRONMENT" == github-hosted && "$(uname -m)" == arm64 ]]
name=Cryptex-M17-Mac-Ilk-Kurulum
if "$RUNNER_TEMP/$name/KUR.command"; then echo "Unexpected account accepted"; exit 1; fi
account=cryptexm14
if id "$account" >/dev/null 2>&1; then exit 1; fi
pw="$(openssl rand -hex 24)"
sudo sysadminctl -addUser "$account" -fullName 'Cryptex M17 trial' -password "$pw"
unset pw
home="$(dscl . -read "/Users/$account" NFSHomeDirectory | cut -d' ' -f2-)"
[[ "$home" == "/Users/$account" ]]
sudo createhomedir -c -u "$account"
run=(sudo -H -u "$account")
[[ " $(id -Gn "$account") " != *' admin '* ]]
sudo install -o "$account" -m 600 "$RUNNER_TEMP/$name.zip" "$home/trial.zip"
sudo install -o "$account" -m 644 scripts/m17/verify_first_install_mac.py "$home/verify.py"
"${run[@]}" /bin/bash -s -- "$BUILD_REVISION" <<'TRIAL'
set -euo pipefail
cd "$HOME"
security=/usr/bin/security
kc="$HOME/Library/Keychains/trial.keychain-db"
mkdir -p "$(dirname "$kc")"
pw="$(/usr/bin/openssl rand -hex 24)"
"$security" create-keychain -p "$pw" "$kc"
"$security" unlock-keychain -p "$pw" "$kc"
"$security" default-keychain -d user -s "$kc"
"$security" list-keychains -d user -s "$kc"
ditto -x -k trial.zip .
dir="$HOME/Cryptex-M17-Mac-Ilk-Kurulum"
report="$dir/M17-IMZA-SONUC-Darwin.json"
app="$HOME/Applications/Cryptex-M17-Ilk-Kurulum/Cryptex.app"
# Headless test accounts have no GUI for the first native Keychain signing prompt.
# Only this runner's empty, disposable keychain is eligible for the test ACL.
if ! "$dir/KUR.command"; then
  [[ -f "$HOME/Library/Application Support/Cryptex-M17-Ilk-Kurulum/owner.der" ]]
  [[ "$(plutil -extract phase raw "$report")" == imza ]]
  "$security" set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$pw" "$kc" >/dev/null
  "$dir/KUR.command"
fi
unset pw
[[ "$(plutil -extract passed raw "$report")" == true ]]
[[ "$(plutil -extract revision raw "$report")" == "$1" ]]
approved="$(shasum -a 256 "$HOME/Library/Application Support/Cryptex-M17-Ilk-Kurulum/owner.der" | cut -d' ' -f1)"
[[ "$(plutil -extract certificate_sha256 raw "$report")" == "$approved" ]]
/usr/bin/python3 -B verify.py "$app" "$approved" "$(plutil -extract installed_desktop_sha256 raw "$report")"
cp "$report" first.json
"$dir/KUR.command"
cmp first.json "$report"
printf x >> "$dir/Resources/Application.zip"
if "$dir/KUR.command"; then echo "Changed installer accepted"; exit 1; fi
[[ "$(plutil -extract phase raw "$report")" == paket ]]
printf '\n' >> "$app/Contents/Resources/notices/frontend.json"
ditto -x -k trial.zip .
if "$dir/KUR.command"; then echo "Changed installation accepted"; exit 1; fi
[[ "$(plutil -extract phase raw "$report")" == dogrulama ]]
echo 'Standard account: native first install, repeat, separate verifier, and both refusals passed.'
TRIAL
