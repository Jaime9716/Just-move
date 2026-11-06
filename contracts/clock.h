/* contracts/clock.h — CONTRATO
 *
 * Reloj monotónico único del sistema. Motor, adaptadores y Godot (vía la API del
 * motor) usan ESTE reloj; nunca Time.get_ticks_usec() ni relojes propios.
 *
 * Relación con la canción (la calcula B2/SongClock cada fotograma):
 *     song_time_s = t_ns * 1e-9 + mono_to_song_s
 * donde mono_to_song_s ya incluye input_offset y audio_offset.
 */
#ifndef JD_CLOCK_H
#define JD_CLOCK_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Implementado en Rust (jd_core::clock::now_ns, std::time::Instant desde una base fija del proceso). */
int64_t jd_now_ns(void);

#ifdef __cplusplus
}
#endif

#endif
