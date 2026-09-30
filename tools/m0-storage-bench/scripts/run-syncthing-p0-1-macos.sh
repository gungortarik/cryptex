#!/usr/bin/env bash
set -euo pipefail

ST="/Applications/Syncthing.app/Contents/Resources/syncthing/syncthing"
STHOME="$HOME/Library/Application Support/Cryptex-Dev/syncthing-p0-1"
CONFIG="$STHOME/config.xml"

fail() {
  echo "error: $*" >&2
  exit 2
}

[[ "$(uname -s)" == "Darwin" ]] || fail "macOS is required"
[[ -x "$ST" ]] || fail "Syncthing binary not found: $ST"
[[ -f "$CONFIG" ]] || fail "config.xml not found: $CONFIG"

GUI_ADDRESS="$(sed -n '/<gui /,/<\/gui>/p' "$CONFIG" | sed -n 's:.*<address>\([^<]*\)</address>.*:\1:p' | head -1)"
[[ "$GUI_ADDRESS" == "127.0.0.1:18384" ]] || fail "unexpected GUI address: $GUI_ADDRESS"

LISTENER_LINES="$(grep '<listenAddress>' "$CONFIG" || true)"
LISTENER_COUNT="$(printf '%s\n' "$LISTENER_LINES" | sed '/^[[:space:]]*$/d' | wc -l | tr -d ' ')"
[[ "$LISTENER_COUNT" == "1" ]] || fail "expected exactly one sync listener, found $LISTENER_COUNT"
printf '%s\n' "$LISTENER_LINES" | grep -q '<listenAddress>tcp://0.0.0.0:22001</listenAddress>' || fail "expected TCP listener tcp://0.0.0.0:22001"

for OPTION in globalAnnounceEnabled localAnnounceEnabled relaysEnabled natEnabled; do
  VALUE="$(sed -n "s:.*<$OPTION>\([^<]*\)</$OPTION>.*:\1:p" "$CONFIG" | head -1)"
  [[ "$VALUE" == "false" ]] || fail "unexpected $OPTION value: ${VALUE:-missing}"
done

if grep -Eq '<folder .*id="[^"]+".*path="[^"]+"' "$CONFIG"; then
  fail "active folder entries already exist; refusing pre-pairing launch"
fi

if lsof -nP -iTCP:18384 -sTCP:LISTEN >/dev/null 2>&1; then
  fail "GUI port 18384 is already in use"
fi
if lsof -nP -iTCP:22001 -sTCP:LISTEN >/dev/null 2>&1; then
  fail "sync port 22001 is already in use"
fi

echo "scope=isolated_p0_1_syncthing_only"
echo "home=$STHOME"
echo "gui=127.0.0.1:18384"
echo "listen_address=tcp://0.0.0.0:22001"
echo "global_announce=false"
echo "local_announce=false"
echo "relays=false"
echo "nat=false"
echo "transport_scope=direct_tcp_only"
echo "active_folder_count=0"
echo "starting_foreground=true"

exec "$ST" serve --home="$STHOME" --no-browser --no-restart
