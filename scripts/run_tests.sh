#!/usr/bin/env bash
# run_tests.sh — todos los tests que se pueden ejecutar en esta máquina. Agente G (B10).
#   cargo test (workspace engine/) + lógica de jd_godot sin Godot + pytest + Godot headless si existe.
# Variables: CARGO_TARGET_DIR (opcional), GODOT=/ruta/a/godot (opcional), PYTHON (por defecto python3).
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
PYTHON="${PYTHON:-python3}"
FAILED=()

run() {
    local title="$1"; shift
    echo ""
    echo "==> $title"
    if ! "$@"; then FAILED+=("$title"); fi
}

run "cargo test (engine/)" cargo test --manifest-path engine/Cargo.toml --workspace
run "cargo test (jd_godot/bridge, sin Godot)" cargo test --manifest-path engine/jd_godot/bridge_test/Cargo.toml

if command -v "$PYTHON" >/dev/null; then
    for d in proto/dev_tests proto/tests tools/tests tests/system/tests; do
        [[ -d "$d" ]] || continue
        echo ""
        echo "==> pytest $d"
        "$PYTHON" -m pytest -q "$d"
        rc=$?
        # 5 = ningún test recogido (carpeta vacía): no cuenta como fallo.
        if [[ $rc -ne 0 && $rc -ne 5 ]]; then FAILED+=("pytest $d"); fi
    done
else
    echo "AVISO: no hay $PYTHON; se omiten los tests de Python."
fi

GODOT="${GODOT:-$(command -v godot || command -v godot4 || true)}"
if [[ -n "$GODOT" && -x "$GODOT" ]]; then
    if [[ -f game/bin/libjd_godot.so ]]; then
        run "godot: importar proyecto" "$GODOT" --headless --path game --import
        cp scripts/ci/check_extension.gd game/_ci_check_extension.gd
        run "godot: prueba de la GDExtension" "$GODOT" --headless --path game -s res://_ci_check_extension.gd -- "$ROOT/data"
        rm -f game/_ci_check_extension.gd
    else
        echo "AVISO: falta game/bin/libjd_godot.so (ejecuta scripts/build_linux.sh); se omite la prueba de la extensión."
    fi
    if [[ -f game/tests/run_tests.gd ]]; then
        run "godot: tests de GDScript" "$GODOT" --headless --path game -s res://tests/run_tests.gd
    fi
else
    echo ""
    echo "AVISO: no se encontró Godot (define GODOT=/ruta/a/godot). Se omiten los tests de Godot;"
    echo "       en la CI de GitHub sí se ejecutan (job 'godot headless')."
fi

echo ""
if [[ ${#FAILED[@]} -gt 0 ]]; then
    echo "FALLARON: ${FAILED[*]}"
    exit 1
fi
echo "Todos los tests ejecutados pasaron."
