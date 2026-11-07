# P8 · Robustez: Wi-Fi saturada, batería baja, pantalla bloqueada

**Criterio:** con Wi-Fi cargada, pérdidas ≤ 5 % y sin huecos > 200 ms (o el juego avisa en naranja); al
bloquear la pantalla / cambiar de app el juego marca "desconectado", sigue sin colgarse y al volver
reconecta en < 5 s; **nunca** se cuelga ni se cierra. Tiempo: 40 min.

## (a) Wi-Fi saturada
1. En otro aparato de la misma Wi-Fi pon un vídeo 4K o una descarga grande (o una videollamada).
2. Cierra el juego y graba 3 min bailando:
   `& "<TOOLS>\jd_record.exe" data\sessions\real\p8_wifi_cargada.jsonl --seconds 180 --note "wifi cargada: 4K en la tele"`
   Apunta el rtt que muestra.
3. `python tests\system\tools\session_stats.py data\sessions\real\p8_wifi_cargada.jsonl --max-gaps 99 --max-loss-pct 5`
   y mira también la lista de huecos (ninguno debería pasar de 200 ms).
4. Juega una canción con la carga puesta: ¿se nota en la puntuación?, ¿sale el RTT en naranja?

## (b) Batería baja
1. Con el móvil < 15 % y **ahorro de batería activado**, graba 3 min como en (a) (`p8_bateria_baja.jsonl`).
2. ¿Baja la tasa de 200 Hz? (`session_stats.py` → `tasa`). ¿El juego muestra la batería?

## (c) Pantalla bloqueada / cambiar de app
1. Juega una canción. A mitad, **bloquea la pantalla** del móvil 5 s y desbloquéala.
2. Otra vez: pulsa **Inicio** (app a segundo plano) 5 s y vuelve.
3. Otra vez: recibe una **llamada** o abre la cámara 5 s (si puedes).
4. Apunta cada vez: ¿avisó el juego?, ¿cuánto tardó en volver?, ¿hizo falta tocar algo?, ¿se colgó algo?

## (d) Movimientos bruscos
1. Con el móvil bien sujeto (¡funda, correa, suelo blando!), sacudidas muy fuertes durante 30 s con
   `jd_record` (`p8_brusco.jsonl`). `session_stats.py` dirá si satura (`|a| máx`, valores fuera de rango).

**Apunta** cada caso en `results.csv` y lo que veas raro en el feedback.
