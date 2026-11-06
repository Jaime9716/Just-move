"""jdproto · prototipo Python del juez v1 (bloques B3–B5).

Implementa contracts/judge_algorithm_v1.md. Módulos:
  config   constantes por defecto y fusión de configuración
  session  carga de sesiones .jsonl (contracts/session_format.md)
  choreo   carga y validación de choreo.json (contracts/song_package.md)
  signal   B3: filtro paso bajo + orientación + aceleración lineal
  features B4: tramas (energía, pico, dirección, cuaternión) y picos
  judge    B5: juez por paso y extremidad
  pipeline judge_song(): B3 → B4 → B5 para jugador y referencia
  synth    generador de datos sintéticos de móvil en la mano / pie
"""

__version__ = "1.0.0"
