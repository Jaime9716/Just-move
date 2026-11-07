# P4 · Repetibilidad de la calibración

**Criterio:** 5 calibraciones seguidas → rango (máx − mín) < 10 ms; el "±" de cada una < 15 ms. Tiempo: 15 min.

1. Juego → Conectar móvil (mano derecha) → menú → **Calibrar entrada**.
2. Sigue los 16 clics sacudiendo el móvil **en seco** a cada clic. Al acabar el juego dice
   `Desfase medido con móvil: +NN ms (± SS ms, K golpes)`. Apunta NN y SS. Pulsa **Guardar**
   (así puedes comprobarlo en `profile.cfg`).
3. Repite 5 veces sin cambiar nada. Apúntalo en `tests\system\results\p4_mano.csv`:
   ```
   intento;offset_ms;stddev_ms
   1;+34;6
   2;+31;5
   ...
   ```
4. Analiza:
   ```powershell
   python tests\system\tools\calib_stats.py tests\system\results\p4_mano.csv
   ```
   (También vale escribir los números: `python tests\system\tools\calib_stats.py 34 31 36 33 35`, o pasar
   copias de `%APPDATA%\Godot\app_userdata\Juego de Baile\profile.cfg` hechas tras cada Guardar.)
5. Repite con el móvil **en el tobillo** (pisotones al clic) → `p4_pie.csv`.
6. Repite con **Espacio** del teclado en lugar del móvil (mide tu propio pulso: si sale igual de disperso,
   la dispersión es tuya, no del móvil).
7. (Opcional) **Calibrar pantalla** 5 veces: apunta el `visual_offset` elegido y analiza igual
   (`--key visual_offset` si pasas copias de `profile.cfg`).

**Apunta** media, rango y tendencia (si va subiendo o bajando con los intentos) en `results.csv`.
