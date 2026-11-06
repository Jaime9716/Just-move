/* contracts/engine_api.h — CONTRATO: API C del motor hacia el juego (Godot)
 *
 * La GDExtension (agente G) envuelve SOLO estas funciones. Godot no ve C++ interno.
 * Implementación: engine/api/engine.cpp (agente C) y engine/api/sources.cpp (agente A).
 * Todas las cadenas son UTF-8. Las funciones que devuelven int: 0 = OK, <0 = error.
 */
#ifndef JD_ENGINE_API_H
#define JD_ENGINE_API_H

#include <stdint.h>
#include "motion_sample.h"
#include "clock.h"

#if defined(_WIN32) && defined(JD_BUILDING_ENGINE)
#  define JD_API __declspec(dllexport)
#elif defined(_WIN32)
#  define JD_API
#else
#  define JD_API __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define JD_ENGINE_API_VERSION 1

JD_API int jd_api_version(void);

/* ---------------------------------------------------------------- Fuentes (B1, B7) */

typedef struct JD_Source JD_Source;

typedef struct JD_DeviceInfoC {
    uint32_t source_id;
    char     name[64];
    char     kind[16];
    uint8_t  limb;       /* JD_Limb */
    uint8_t  connected;  /* 0/1 */
    float    battery;    /* 0..1, -1 desconocido */
} JD_DeviceInfoC;

/* Joy-Con reales vía SDL3. Si el motor se compiló sin SDL3 devuelve NULL. */
JD_API JD_Source* jd_source_create_joycon(void);

/* Reproduce una sesión grabada (formato contracts/session.schema.json).
 * Las muestras se emiten en tiempo real a partir de start(), con t_ns = jd_now_ns()
 * del instante equivalente. speed = 1.0 tiempo real. */
JD_API JD_Source* jd_source_create_replay(const char* session_path, double speed);

/* Envuelve otra fuente y graba todo lo que pasa por poll() en out_path (JSONL).
 * mono_to_song_s: se actualiza con jd_source_recorder_set_mapping para guardar en tiempo de canción. */
JD_API JD_Source* jd_source_create_recorder(JD_Source* inner, const char* out_path);
JD_API void       jd_source_recorder_set_mapping(JD_Source* rec, double mono_to_song_s);

JD_API int  jd_source_start(JD_Source* s);
JD_API void jd_source_stop(JD_Source* s);
/* Copia hasta max muestras nuevas a out. Devuelve cuántas (>=0) o <0 si error. */
JD_API int  jd_source_poll(JD_Source* s, JD_MotionSample* out, int max);
JD_API int  jd_source_device_count(JD_Source* s);
JD_API int  jd_source_device_info(JD_Source* s, int index, JD_DeviceInfoC* out);
JD_API void jd_source_assign_limb(JD_Source* s, uint32_t source_id, uint8_t limb);
JD_API const char* jd_source_last_error(JD_Source* s);
JD_API void jd_source_destroy(JD_Source* s);

/* ---------------------------------------------------------------- Motor (B3–B6) */

typedef struct JD_Engine JD_Engine;

typedef enum JD_Grade {
    JD_GRADE_MISS    = 0,
    JD_GRADE_GOOD    = 1,
    JD_GRADE_PERFECT = 2
} JD_Grade;

typedef struct JD_JudgeConfig {
    float perfect_window_ms;  /* por defecto 50  */
    float good_window_ms;     /* por defecto 100 */
    float foot_window_scale;  /* multiplica ventanas para pies, por defecto 1.5 */
} JD_JudgeConfig;

typedef struct JD_StepResult {
    int32_t step_index;       /* índice en choreo.steps */
    uint8_t limb;             /* JD_Limb evaluada (un resultado por extremidad del paso) */
    uint8_t grade;            /* JD_Grade */
    uint16_t _reserved;
    float   similarity;       /* 0..1 */
    float   timing_error_ms;  /* + tarde, - pronto; 0 si no aplica */
} JD_StepResult;

JD_API JD_Engine* jd_engine_create(void);
JD_API void       jd_engine_destroy(JD_Engine* e);
JD_API void       jd_engine_default_config(JD_JudgeConfig* out);
JD_API void       jd_engine_set_config(JD_Engine* e, const JD_JudgeConfig* cfg);

/* Carga un paquete de canción (carpeta con choreo.json, ver contracts/song_package.md).
 * err recibe un mensaje legible si falla. */
JD_API int jd_engine_load_song(JD_Engine* e, const char* song_dir, char* err, int err_len);

/* Relación reloj monotónico -> canción. Llamar cada fotograma desde SongClock. */
JD_API void jd_engine_set_time_mapping(JD_Engine* e, double mono_to_song_s);

JD_API void jd_engine_push_samples(JD_Engine* e, const JD_MotionSample* s, int count);

/* Avanza el juez hasta song_time_s. Escribe en out los resultados de los pasos
 * cuya ventana ya se cerró (hasta max_out). Devuelve cuántos escribió. */
JD_API int  jd_engine_update(JD_Engine* e, double song_time_s, JD_StepResult* out, int max_out);

/* Vuelve al inicio de la canción (descarta muestras y resultados pendientes). */
JD_API void jd_engine_reset(JD_Engine* e);

#ifdef __cplusplus
}
#endif

#endif /* JD_ENGINE_API_H */
