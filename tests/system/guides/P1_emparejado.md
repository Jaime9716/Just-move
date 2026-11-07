# P1 · Emparejado y reconexión del móvil

**Criterio:** empareja en < 30 s la 1ª vez; un código erróneo se rechaza; tras perder la Wi-Fi 10 s
reconecta en < 5 s **sin reiniciar el juego** y conserva la extremidad; nada se cuelga. Tiempo: 15 min.

1. Abre el juego → **Conectar móvil**. Apunta IP:puerto y código. Pon el cronómetro en marcha.
2. **Código erróneo**: en la app escribe otro código → debe decir "Código incorrecto" y el juego no lo lista
   como conectado.
3. **Emparejado**: escribe el código bueno → "Conectado como mano derecha". Para el cronómetro (medida 1).
   Apunta el RTT del juego (verde < 30 ms, naranja > 30 ms) y la batería.
4. **Descubrimiento**: cierra la app, ábrela: ¿rellenó sola la IP (DISCOVER)? (sí/no).
5. **Cambio de extremidad**: en el juego cambia el móvil a "Pie izquierdo" → la app debe mostrar el cambio.
   Vuelve a "Mano derecha".
6. **Pérdida de Wi-Fi**: en el móvil desactiva la Wi-Fi. A los ~3 s el juego debe marcarlo
   **desconectado**. Espera 10 s y reactívala; cronometra hasta que vuelve a "conectado" (medida 2).
   ¿Conserva la extremidad? ¿Hizo falta tocar algo en la app?
7. **App a segundo plano**: pulsa Inicio en el móvil 10 s y vuelve a la app. Cronometra la reconexión (medida 3).
8. **Cerrar y abrir la app** del todo (quitarla de recientes): ¿vuelve con la misma extremidad? (medida 4).
9. **Segundo móvil** (si tienes): emparéjalo → debe salir como "mano izquierda".
10. Durante todo esto, el juego no debe congelarse ni dar error.

**Apunta** en `results.csv`: `P1,<fecha>,<móvil/Wi-Fi>,"empareja 12 s; reconecta 3 s; extremidad ok",...`.
Si la reconexión falla, repite el paso 6 con `jd_record` en vez del juego (cierra el juego) y guarda la
sesión: `& "<TOOLS>\jd_record.exe" data\sessions\real\p1_reconexion.jsonl --seconds 60` — el hueco se ve con
`python tests\system\tools\session_stats.py data\sessions\real\p1_reconexion.jsonl`.
