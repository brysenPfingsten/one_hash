#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
oh="${OH_BIN:-$repo_root/target/release/one_hash}"
diag_asm="$repo_root/examples/diag.asm"

if [[ ! -x "$oh" ]]; then
  (cd "$repo_root" && cargo build --release -q)
fi

x="$("$oh" --asm "$diag_asm")"
y="$("$oh" -r1 "$x" -e "$x" | awk '/^  R1: /{print $2; exit}')"
echo $y
