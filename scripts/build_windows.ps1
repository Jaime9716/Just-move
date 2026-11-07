# build_windows.ps1 — compila el motor y la GDExtension en Windows x86-64 y los deja listos en game/bin.
# Agente G (B10). Uso, desde la raíz del repo en PowerShell:
#     powershell -ExecutionPolicy Bypass -File scripts\build_windows.ps1            # compila + tests
#     powershell -ExecutionPolicy Bypass -File scripts\build_windows.ps1 -SkipTests
# Requisitos: ver BUILD.md (Rust con toolchain MSVC + Build Tools de Visual Studio; Python+numpy y
# Godot 4.4 opcionales para los tests).
param(
    [switch]$SkipTests
)

$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

function Invoke-Step([string]$Title, [scriptblock]$Block) {
    Write-Host ""
    Write-Host "==> $Title" -ForegroundColor Cyan
    & $Block
    if ($LASTEXITCODE -ne 0) {
        Write-Host "ERROR en: $Title (código $LASTEXITCODE)" -ForegroundColor Red
        exit $LASTEXITCODE
    }
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "No se encuentra 'cargo'. Instala Rust con rustup (ver BUILD.md) y abre una terminal nueva." -ForegroundColor Red
    exit 1
}

Invoke-Step "Motor (workspace engine/): cargo build --release" {
    cargo build --release --manifest-path engine/Cargo.toml --workspace
}

Invoke-Step "GDExtension (engine/jd_godot): cargo build --release" {
    cargo build --release --manifest-path engine/jd_godot/Cargo.toml
}

$Bin = Join-Path $Root "game\bin"
New-Item -ItemType Directory -Force -Path $Bin | Out-Null
$Dll = Join-Path $Root "engine\jd_godot\target\release\jd_godot.dll"
$Pdb = Join-Path $Root "engine\jd_godot\target\release\jd_godot.pdb"
try {
    Copy-Item $Dll $Bin -Force
} catch {
    Write-Host "No se pudo copiar jd_godot.dll a game\bin. ¿Está Godot abierto? Ciérralo y repite." -ForegroundColor Red
    exit 1
}
if (Test-Path $Pdb) { Copy-Item $Pdb $Bin -Force -ErrorAction SilentlyContinue }
Write-Host "Copiado: game\bin\jd_godot.dll" -ForegroundColor Green
Write-Host "Herramientas: engine\target\release\jd_record.exe, phone_sim.exe, jd_judge.exe"

if ($SkipTests) { exit 0 }

Invoke-Step "Tests del motor: cargo test" {
    cargo test --manifest-path engine/Cargo.toml --workspace
}
Invoke-Step "Tests de jd_godot (sin Godot)" {
    cargo test --manifest-path engine/jd_godot/bridge_test/Cargo.toml
}

$Py = Get-Command python -ErrorAction SilentlyContinue
if ($Py) {
    foreach ($d in @("proto/dev_tests", "proto/tests", "tools/tests", "tests/system/tests")) {
        if (Test-Path $d) {
            Write-Host ""
            Write-Host "==> pytest $d" -ForegroundColor Cyan
            python -m pytest -q $d
            if ($LASTEXITCODE -ne 0 -and $LASTEXITCODE -ne 5) { Write-Host "Fallan los tests de $d" -ForegroundColor Red; exit 1 }
        }
    }
} else {
    Write-Host "AVISO: no hay 'python' en el PATH; se omiten los tests de Python." -ForegroundColor Yellow
}

$Godot = $env:GODOT
if (-not $Godot) {
    $g = Get-Command godot -ErrorAction SilentlyContinue
    if ($g) { $Godot = $g.Source }
}
if ($Godot -and (Test-Path $Godot)) {
    Invoke-Step "Godot: importar el proyecto" { & $Godot --headless --path game --import }
    Copy-Item scripts\ci\check_extension.gd game\_ci_check_extension.gd -Force
    Invoke-Step "Godot: prueba de la GDExtension" {
        & $Godot --headless --path game -s res://_ci_check_extension.gd -- (Join-Path $Root "data")
    }
    Remove-Item game\_ci_check_extension.gd -ErrorAction SilentlyContinue
    if (Test-Path game\tests\run_tests.gd) {
        Invoke-Step "Godot: tests de GDScript" { & $Godot --headless --path game -s res://tests/run_tests.gd }
    }
} else {
    Write-Host "AVISO: no se encontró Godot (define `$env:GODOT con la ruta al .exe, p. ej. la versión _console.exe); se omiten sus tests." -ForegroundColor Yellow
}
Write-Host ""
Write-Host "Todo listo. Abre game/project.godot con Godot 4.4." -ForegroundColor Green
