/* contracts/motion_sample.h — CONTRATO (solo lo cambia el arquitecto)
 *
 * Una muestra de movimiento normalizada. Todas las fuentes (Joy-Con, reproducción
 * de sesiones, en el futuro webcam/móvil/ESP32) producen exactamente esto.
 */
#ifndef JD_MOTION_SAMPLE_H
#define JD_MOTION_SAMPLE_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Extremidad a la que está atado el sensor. */
typedef enum JD_Limb {
    JD_LIMB_LEFT_HAND  = 0,
    JD_LIMB_RIGHT_HAND = 1,
    JD_LIMB_LEFT_FOOT  = 2,
    JD_LIMB_RIGHT_FOOT = 3,
    JD_LIMB_COUNT      = 4,
    JD_LIMB_NONE       = 255
} JD_Limb;

/* Nombres en archivos JSON: "left_hand", "right_hand", "left_foot", "right_foot". */

enum {
    JD_SAMPLE_HAS_ACCEL = 1u << 0,
    JD_SAMPLE_HAS_GYRO  = 1u << 1
};

typedef struct JD_MotionSample {
    /* Instante en que OCURRIÓ el movimiento, en el reloj monotónico del host
     * (ver contracts/clock.h, jd_now_ns). El adaptador ya ha restado la
     * antigüedad de la muestra (p. ej. SDL_GetTicksNS() - sensor_timestamp). */
    int64_t  t_ns;
    uint32_t source_id;  /* id del dispositivo, único dentro de su fuente */
    uint8_t  limb;       /* JD_Limb */
    uint8_t  flags;      /* JD_SAMPLE_HAS_* */
    uint16_t _reserved;
    /* Unidades SI y ejes tal como los reporta SDL3 para mandos:
     * accel en m/s^2 (incluye gravedad), gyro en rad/s.
     * Ejes del mando sujetado como un gamepad: X derecha, Y arriba, Z hacia el jugador. */
    float accel[3];
    float gyro[3];
} JD_MotionSample;

#ifdef __cplusplus
}
#endif

#endif /* JD_MOTION_SAMPLE_H */
