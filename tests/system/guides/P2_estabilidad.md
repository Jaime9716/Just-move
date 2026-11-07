# P2 · Estabilidad de muestras y pérdidas por Wi-Fi

**Criterio:** por móvil 200 Hz ± 5 %, **0 huecos > 50 ms** (≤ 2 en 10 min si estás lejos del router),
pérdidas ≤ 1 %, `t` siempre creciente, sin valores imposibles, RTT < 30 ms. Tiempo: ~45 min.

Cierra el juego (usa el mismo puerto que `jd_record`).

| Sesión | Situación | Comando (`--note` con lo que hiciste) |
|---|---|---|
| A | Reposo, móvil sobre la mesa, a ~3 m del router | `& "<TOOLS>\jd_record.exe" data\sessions\real\p2_reposo_3m.jsonl --seconds 600 --note "reposo 3 m 5 GHz"` |
| B | En movimiento (baila/agita con la mano), ~3 m | `... p2_mov_3m.jsonl --seconds 600 --note "movimiento 3 m"` |
| C | En movimiento, otra habitación / ~10 m | `... p2_mov_10m.jsonl --seconds 600 --note "movimiento 10 m, una pared"` |
| D (opcional) | Igual que B en la otra banda (2,4 GHz ↔ 5 GHz) | `... p2_mov_3m_24ghz.jsonl --seconds 600` |

1. Lanza el comando, conecta el móvil, y deja que grabe los 10 min. Mira de vez en cuando la línea de
   `jd_record` (Hz, rtt, perdidos) y apunta el **rtt** típico y el máximo que veas.
2. Analiza cada sesión:
   ```powershell
   python tests\system\tools\session_stats.py data\sessions\real\p2_*.jsonl
   ```
   Lee por dispositivo: tasa, `huecos`, peor Δt, pérdidas, `t monotónico`, rangos y `=> APROBADO/SUSPENDIDO`.
   En la sesión C usa `--max-gaps 2`. En reposo, `|a|` debería ser ≈ 9,8 m/s².
3. Guarda la salida completa para mí: `python tests\system\tools\session_stats.py data\sessions\real\p2_*.jsonl --json > tests\system\results\p2.json`.
4. Una fila por sesión en `results.csv`.

Si hay huecos: ¿coinciden con algo (microondas, alguien viendo vídeo, el móvil se bloqueó)? Anótalo.
