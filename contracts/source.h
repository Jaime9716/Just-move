// contracts/source.h — CONTRATO (C++ interno del motor)
//
// Interfaz común de todos los adaptadores de fuente (bloque B1) y de la
// reproducción de sesiones (bloque B7). El resto del motor solo ve esto.
#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

#include "motion_sample.h"

namespace jd {

struct DeviceInfo {
    uint32_t    source_id = 0;
    std::string name;            // p. ej. "Joy-Con (L)"
    std::string kind;            // "joycon", "replay", "fake", ...
    JD_Limb     limb = JD_LIMB_NONE;
    bool        connected = false;
    float       battery = -1.f;  // 0..1, -1 si se desconoce
};

class IMotionSource {
public:
    virtual ~IMotionSource() = default;

    // Abre dispositivos / archivos. false si falla (ver last_error()).
    virtual bool start() = 0;
    virtual void stop() = 0;

    // Añade a `out` todas las muestras disponibles desde la última llamada,
    // ordenadas por t_ns creciente por dispositivo. Devuelve cuántas añadió.
    // Se llama desde un único hilo (normalmente una vez por fotograma).
    virtual size_t poll(std::vector<JD_MotionSample>& out) = 0;

    virtual std::vector<DeviceInfo> devices() const = 0;
    virtual void assign_limb(uint32_t source_id, JD_Limb limb) = 0;

    virtual std::string last_error() const { return {}; }
};

} // namespace jd
