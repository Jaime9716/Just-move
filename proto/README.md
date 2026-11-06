# proto/ · Prototipo Python del juez (H0)

Implementación de referencia de **contracts/judge_algorithm_v1.md** (bloques B3 señal, B4 rasgos,
B5 juez) más un generador de datos sintéticos de **móvil Android en la mano** (y en el pie).
Sus salidas en `data/sessions/synthetic/golden/` son las que el motor Rust (agente C) debe reproducir.

Requisitos: Python 3.11+ y numpy (pytest para los tests). Sin más dependencias.

```
proto/
  jdproto/            paquete (API del contrato)
    config.py         constantes v1 + variantes ("v1b") + merge_config()
    session.py        load_session()            (contracts/session_format.md)
    choreo.py         load_choreo()             (contracts/song_package.md, con validación)
    signal.py         B3: paso bajo, orientación, lin_accel
    features.py       B4: tramas y picos
    judge.py          B5: accent / move con y sin referencia
    pipeline.py       judge_song()
    judge_cli.py      CLI
    synth.py          modelo de movimiento del móvil y sesiones sintéticas
  synth.py            script que regenera data/sessions/synthetic/ (determinista)
  dev_tests/          tests propios (proto/tests/ es de T1)
  ALGORITHM.md        precisiones de v1, problemas encontrados y variante v1b
```

## Ejecutar

```bash
cd <repo>
python3 proto/synth.py                       # regenera canción demo, sesiones y golden (≈10 s)
python3 proto/synth.py --variant v1b         # igual, y muestra también la tabla con v1b
python3 -m pytest -q proto/dev_tests         # tests propios (≈5 s)

# juzgar una sesión (tabla legible o JSON = formato golden)
PYTHONPATH=proto python3 -m jdproto.judge_cli data/sessions/synthetic/song_demo \
    data/sessions/synthetic/late_40ms.jsonl
PYTHONPATH=proto python3 -m jdproto.judge_cli data/sessions/synthetic/song_demo \
    data/sessions/synthetic/late_40ms.jsonl --json
```

Opciones extra del CLI: `--no-ref` (ignora `ref_imu.jsonl`), `--variant v1b`, `--config cfg.json`
(claves de `jdproto.config.DEFAULT_CONFIG`, p. ej. `{"peak_min": 9}`), `--offset S` (ver abajo).

Desde Python:

```python
from jdproto.session import load_session
from jdproto.choreo import load_choreo
from jdproto.pipeline import judge_song
c = load_choreo("data/sessions/synthetic/song_demo")
ref = load_session("data/sessions/synthetic/song_demo/ref_imu.jsonl")["samples"]
me = load_session("data/sessions/synthetic/perfect.jsonl")["samples"]
res = judge_song(c, me, ref, None)            # o {"variant": "v1b"}, {"PEAK_MIN": 9}, ...
```

## Juzgar una sesión real grabada con `jd_record`

1. Graba con `jd_record` (agente A, `engine/jd_tools`) mientras bailas la canción con el móvil en la
   mano (pantalla hacia arriba). Obtienes un `.jsonl` con el formato de `contracts/session_format.md`.
2. Ten el paquete de la canción (`data/songs/<song_id>/` con `choreo.json` y, si existe,
   `ref_imu.jsonl`).
3. Si la cabecera dice `"time_base":"song"`, el tiempo ya es de canción:
   ```bash
   PYTHONPATH=proto python3 -m jdproto.judge_cli data/songs/<song_id> mi_sesion.jsonl
   ```
   Si dice `"time_base":"mono"` (segundos desde el inicio de la grabación), indica el tiempo de canción
   que corresponde a t = 0 de la grabación: `--offset S` (t_song = t + S). Por ejemplo, si empezaste a
   grabar 3.2 s antes de que sonara la canción, `--offset -3.2`. Sin `--offset` se asume 0 y se avisa.
4. Para crear una **referencia** desde una grabación tuya bien bailada: guárdala (en tiempo de canción)
   como `data/songs/<song_id>/ref_imu.jsonl` y pon `"references":{"imu":"ref_imu.jsonl"}` en el
   `choreo.json`. Sin referencia, los moves se juzgan solo por energía (nunca Perfect).
5. Si una sesión real da notas raras, consérvala: según docs/06 se convierte en test de regresión.

## Datos sintéticos (`data/sessions/synthetic/`)

`song_demo/` (choreo.json de 30 s a 120 BPM con 20 pasos, 27 resultados por sesión: golpes `punch_up`,
`punch_forward`, `stomp`; moves `wave`, `pump`, `circle`, `swing`, `side_step`; manos y pie derecho;
`ref_imu.jsonl` con 3 móviles). Sesiones del jugador (3 móviles: mano izq., mano der., pie der.):
`perfect`, `late_40ms`, `late_150ms`, `early_80ms`, `wrong_moves`, `still`, `noisy`, `foot_only`
(1 móvil en el pie). `golden/<sesión>.json` = salida de `judge_song` con v1 (+ `perfect_noref.json`,
la sesión perfect sin referencia). El modelo físico está descrito en `jdproto/synth.py`.

Las marcas de tiempo llevan una fase por dispositivo y jitter (±0.4 ms) para que ninguna muestra caiga
justo en el borde de una trama (evita diferencias Python/Rust por redondeo). No hay audio: `audio.ogg`
no existe en el paquete demo (el juez no lo necesita).
