# 07 · Plan de pruebas de alto nivel (sistema y mundo real)

Comprueba el juego completo **con Joy-Con reales, en tu PC y bailando**. El agente **T3** prepara cada
prueba (guía paso a paso, scripts de medida, hoja de resultados) y analiza los datos; **tú la ejecutas**
y das tu feedback (ver `docs/feedback/`).

| ID | Prueba | Procedimiento | Criterio de aceptación | Hito |
|---|---|---|---|---|
| P1 | Emparejado | Emparejar los dos Joy-Con en Windows; apagar uno y reconectarlo | Detectados y reasignados sin reiniciar | H1 |
| P2 | Estabilidad de muestras | Grabar 10 min en reposo y en movimiento | ~200 Hz por mando, sin huecos > 50 ms | H1 |
| P3 | Sincronía audio–imagen | Grabar pantalla y sonido con la cámara lenta del móvil, canción con destellos al ritmo | Desfase ≤ 10 ms al principio y al final | H2 |
| P4 | Repetibilidad de calibración | Calibrar 5 veces seguidas | Offsets varían < 10 ms | H2 |
| P5 | Justicia de la puntuación | Bailar bien, a destiempo, pasos equivocados y quieto | Bien → Perfect/Good; mal o quieto → Miss | H0, H3 |
| P6 | Otra persona | Alguien nuevo baila la coreografía dos veces | Mejora la 2ª vez; le parece justa | H3 |
| P7 | Joy-Con en el pie | Repetir P5 con un Joy-Con en el tobillo | Puntuación del pie coherente | H3 |
| P8 | Robustez física | Correa floja, movimientos bruscos, batería baja | Avisa o tolera; no se cuelga | H4 |
| P9 | Rendimiento | Canción completa midiendo fps | 60 fps estables | H4 |
| P10 | Primera vez | De abrir el juego a terminar una canción, con calibración | Sin ayuda, < 5 min | H4 |
| P11 | Linux | Repetir P1–P5 en Linux | Mismos criterios | H5 |
