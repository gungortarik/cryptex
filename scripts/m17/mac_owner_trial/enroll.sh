#!/bin/bash
set -euo pipefail
umask 077
export PATH=/usr/bin:/bin:/usr/sbin:/sbin
[[ "$(id -un)" == cryptexm14 && "$(id -u)" != 0 ]]
kit="$(cd -P -- "$(dirname -- "$0")" && pwd)"
tmp="$1"
cert="$HOME/Library/Application Support/Cryptex-M17-Ilk-Kurulum/owner.der"
[[ -d "$tmp" && ! -L "$tmp" && ! -e "$cert" && ! -L "$cert" ]]
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
