# Tests de GDScript del juego (agente T2)

Runner propio sin dependencias (`run_tests.gd`, no hace falta GUT). Prueba la lógica pura de
`contracts/game_logic_api.md`, las simulaciones del motor (`MockJDEngine`/`MockJDSource`/`EngineFactory`),
`SongLibrary`, la interfaz de la GDExtension (si está cargada) y, como humo, las escenas `main_menu` y
`gameplay`.

## Ejecutar en Windows

Desde la raíz del repositorio (PowerShell o cmd), con Godot 4.4.x (`godot.exe` o
`Godot_v4.4.1-stable_win64_console.exe`, que muestra la salida en la consola):

```powershell
# 1) Solo la primera vez, o después de añadir/renombrar clases: importa el proyecto
#    (genera game/.godot/ con la caché de class_name; sin ella todo sale "SKIP").
godot.exe --headless --path game --import

# 2) Ejecutar los tests
godot.exe --headless --path game -s res://tests/run_tests.gd
echo $LASTEXITCODE        # PowerShell: 0 = todo OK, 1 = algún fallo   (cmd: echo %ERRORLEVEL%)

# Solo los tests cuyo nombre contiene un texto, mostrando también los que pasan:
godot.exe --headless --path game -s res://tests/run_tests.gd -- --filter=test_profile --verbose
```

Si `godot.exe` no está en el PATH, usa la ruta completa, p. ej.
`& "C:\Godot\Godot_v4.4.1-stable_win64_console.exe" --headless --path game -s res://tests/run_tests.gd`.
Usa el ejecutable `_console.exe`: el normal no escribe en la consola de Windows.

Con la GDExtension compilada en `game/bin/jd_godot.dll` también se prueban las clases nativas
(`test_native_extension.gd` y los tests "motor de la fábrica"); sin ella esos tests salen como SKIP o usan
la simulación.

Linux / CI: igual con `godot` (el job `godot` de `.github/workflows/ci.yml` ya lo hace tras `--import`).

## Qué significa la salida

- `FAIL archivo::test` + líneas con cada assert que falló. Al final, resumen y lista de fallos.
- `SKIP archivo (faltan clases: X)`: el archivo declara `# requires: X` y esa clase no existe.
- `SKIP archivo::test (motivo)`: el test se omitió en ejecución (sin extensión, sin autoload…).
- `no compila`: el archivo de test (o una clase que usa) tiene errores; míralos más arriba en la salida.
- `no llegó a done()`: el test se cortó por un error de script en tiempo de ejecución (mira arriba).
- La prueba de humo de escenas no puede capturar los `ERROR:` del registro de Godot 4.4: revísalos a ojo.

Los tests solo escriben en `user://t2_tests/` (se borra al terminar). Nunca tocan `user://profile.cfg`.

## Escribir un test

```gdscript
# requires: MiClase
extends "res://tests/test_base.gd"

func test_algo() -> void:
	assert_eq(MiClase.doble(2), 4, "doble")
	assert_near(MiClase.mitad(1.0), 0.5, 1e-9)
	done()      # obligatorio al final
```

Asserts: `assert_true/false`, `assert_eq/ne` (int y float se comparan como números), `assert_near`,
`assert_lt/le/gt/ge`, `assert_null/not_null`, `assert_type`, `assert_has`, `assert_has_method`, `fail`.
`skip("motivo")` + `return` para omitir. `before_each()`/`after_each()`. Tests por fotogramas:
`func step_test_x(frame: int) -> bool` (devuelve true al acabar; límite `max_frames()`). Sin `await`.
