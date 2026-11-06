# Juego de Baile — motor de movimiento

Juego de baile cuyo motor funciona con Joy-Con (y en el futuro webcam, móvil o sensores IMU).
Prioridad actual: **Windows x86-64 + Joy-Con**, después Linux.

## Estructura

```
docs/
  01-arquitectura.md         Visión general, capas y tecnología
  02-sincronizacion.md       Cómo se sincronizan imagen, música y Joy-Con
  03-requisitos-e-hitos.md   Requisitos mínimos, decisiones, hitos y riesgos
  04-plan-de-trabajo.md      Plan de trabajo en paralelo por agentes
  05-arquitectura-bloques.md Los 10 bloques lógico-funcionales
  06-plan-pruebas-bajo-nivel.md  Tests de software (agentes T1, T2)
  07-plan-pruebas-alto-nivel.md  Pruebas reales con Joy-Con (agente T3 + tú)
  feedback/                  Tu feedback del mundo real (plantilla incluida)
src/
  godot/SongClock.gd         Reloj maestro basado en el audio
```

## Estado

Hito actual: **H0 · Prototipo en Python** (ver docs/03-requisitos-e-hitos.md).

Documento vivo de requisitos: https://claude.ai/artifact/NnR4oVqUA3CGrU3jsbaLLc
