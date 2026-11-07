#!/usr/bin/env bash
# build_linux.sh — compila el motor y la GDExtension en Linux x86-64 y copia libjd_godot.so a game/bin.
# Agente G (B10). Uso: scripts/build_linux.sh [--skip-tests]
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

command -v cargo >/dev/null || { echo "No se encuentra 'cargo': instala Rust con rustup (ver BUILD.md)." >&2; exit 1; }

echo "==> Motor (workspace engine/): cargo build --release"
cargo build --release --manifest-path engine/Cargo.toml --workspace

echo "==> GDExtension (engine/jd_godot): cargo build --release"
cargo build --release --manifest-path engine/jd_godot/Cargo.toml

# Respeta CARGO_TARGET_DIR si está definido (relativo = respecto a la raíz del repo).
LIB_DIR="${CARGO_TARGET_DIR:-engine/jd_godot/target}/release"
mkdir -p game/bin
cp "$LIB_DIR/libjd_godot.so" game/bin/
echo "Copiado: game/bin/libjd_godot.so"
echo "Herramientas: ${CARGO_TARGET_DIR:-engine/target}/release/{jd_record,phone_sim,jd_judge}"

if [[ "${1:-}" != "--skip-tests" ]]; then
    "$ROOT/scripts/run_tests.sh"
fi
