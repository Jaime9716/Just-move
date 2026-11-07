# P7 · Móvil en el pie / tobillo

**Criterio:** mismo orden que P5 (bien > mal ≥ quieto); bien → ≥ 60 % Perfect+Good; quieto → ≥ 95 % Miss;
el móvil no se suelta durante una canción. Tiempo: 30 min.

1. Ata el móvil al **tobillo derecho**, pantalla hacia fuera y arriba del pie (calcetín alto, cinta de
   correr para el brazo o brazalete con velcro). **Funda puesta**, suelo blando (alfombra) y sin zapatos
   duros: evita golpes. Apunta cómo lo ataste.
2. Juego → Conectar móvil → cambia la extremidad a **Pie derecho**.
3. Calibrar entrada con pisotones (P4, 1–3 intentos; apunta el desfase).
4. Repite las 4 pasadas de P5 (bien / destiempo / equivocado / quieto) con la demo. La demo tiene pasos
   de pie; los de mano no los puedes hacer (saldrán Miss): apunta solo los del pie si la pantalla los
   separa, o el total y dime cuántos pasos de pie había.
5. Graba además una sesión de pie con `jd_record` (cierra el juego) para ver la señal:
   ```powershell
   & "<TOOLS>\jd_record.exe" data\sessions\real\p7_pie-der_bien.jsonl --seconds 40 --note "pie derecho, tobillo, calcetín"
   python tests\system\tools\session_stats.py data\sessions\real\p7_pie-der_bien.jsonl
   ```
   Fíjate en `|a| máx` y en valores fuera de rango (un pisotón fuerte puede saturar el acelerómetro).
6. Feedback: ¿se movió o se soltó?, ¿molesta?, ¿la puntuación del pie es coherente con lo que hiciste?
