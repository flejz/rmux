#!/usr/bin/env sh
set -eu

if [ ! -d crates ]; then
  echo "No crates directory found."
  exit 0
fi

matches="$(find crates -type f -name '*.rs' 2>/dev/null \
  | grep -v '/target/' \
  | grep -v '/tests/' \
  | grep -v '/test/' \
  | grep -v '/src/.*tests' \
  | xargs grep -In -E 'https?://|TcpListener|TcpStream|UdpSocket' 2>/dev/null \
  | grep -v 'https://example\.com' || true)"
if [ -n "$matches" ]; then
  echo "$matches"
  echo "Possible network/runtime references found." >&2
  exit 1
fi

echo "No runtime network references found in crates/*/src."
