# 05 · Arquitectura por bloques lógico-funcionales

> Copia local de la sección del documento vivo: https://claude.ai/artifact/NnR4oVqUA3CGrU3jsbaLLc

El sistema se divide en **10 bloques**. Cada uno tiene una sola responsabilidad y habla con los demás
solo a través de su interfaz en `contracts/`. Se puede cambiar un bloque (p. ej. B1 Joy-Con por B1 webcam)
sin tocar el resto, y cada bloque se prueba por separado.

```
 ┌────────────┐      ┌────────────────┐      ┌──────────────────┐
 │B7 Sesiones │◄────►│B1 Adquisición  │◄─────│B9 Configuración* │
 └────────────┘      └───────┬────────┘      └────────┬─────────┘
                             ▼                        ▼
                     ┌────────────────┐      ┌──────────────────┐
                     │B3 Señal        │      │B2 Tiempo*        │
                     └───────┬────────┘      └────────┬─────────┘
                             ▼                        │ tiempo de
                     ┌────────────────┐               │ canción
                     │B4 Rasgos       │               │
                     └───────┬────────┘               │
 ┌────────────┐              ▼                        │
 │B6 Coreogr. │────► ┌────────────────┐               │
 └─────┬──────┘      │B5 Juez         │◄──────────────┤
       │             └───────┬────────┘               │
       │                     ▼                        │
       └───────────► ┌────────────────┐               │
                     │B8 Presentación*│◄──────────────┘
                     └────────────────┘
 ┌───────────────────────────────────────────────────────────────┐
 │ B10 Plataforma: SDL3, GDExtension, build y CI                 │
 └───────────────────────────────────────────────────────────────┘
 * = Godot; el resto, motor C++ (librería sin Godot)
```

| Bloque | Responsabilidad | Entradas → salidas | Dónde vive | Requisitos | Agente |
|---|---|---|---|---|---|
| B1 · Adquisición | Leer sensores con adaptadores de interfaz común | Joy-Con (SDL3) → muestras con `t_ns` y extremidad | Motor C++ | F1–F4 | A |
| B2 · Tiempo y sincronía | Reloj de canción, conversión de marcas de tiempo, offsets | Posición del audio + offsets → `audible()`, `visual()`, `input_to_song()` | Godot | F6, N3, N4, N7 | D |
| B3 · Procesado de señal | Filtrar, unificar ejes, estimar orientación | Muestras brutas → filtradas + orientación | Motor C++ | F7 | C |
| B4 · Rasgos | Energía, picos, dirección, orientación por extremidad | Muestras filtradas → rasgos por ventana | Motor C++ | F8 | C |
| B5 · Juez | Comparar rasgos con la referencia en la ventana del beat | Rasgos + paso + tiempo → Perfect / Good / Miss | Motor C++ | F10, N6 | C (algoritmo de B) |
| B6 · Coreografía | Formato, validación, carga y grabador | JSON ↔ pasos y referencias | Motor C++ + herramienta | F9, F11 | F |
| B7 · Sesiones | Grabar y reproducir muestras; sustituye a B1 en pruebas | Muestras ↔ JSONL | Motor C++ | F5, N9 | A |
| B8 · Presentación | Música, iconos de pasos, bailarín, menús, resultados | Tiempo + resultados → pantalla y sonido | Godot | F3, F14, N5 | E |
| B9 · Configuración | Calibración, asignación de extremidades, preferencias, guardado | Jugador ↔ perfil | Godot | F12, F13, F15 | D |
| B10 · Plataforma | SDL3, enlace GDExtension, build y CI | — | Ambos | N1, N2, N8, N10 | G |
