// contracts/engine_internal.h — CONTRATO entre bloques internos del motor (C++17)
//
// B3 Señal -> B4 Rasgos -> B5 Juez, con B6 Coreografía. Cada bloque implementa su parte
// en su carpeta; los demás solo dependen de estas declaraciones.
#pragma once

#include <array>
#include <cstdint>
#include <memory>
#include <string>
#include <vector>

#include "motion_sample.h"
#include "engine_api.h"

namespace jd {

// ------------------------------------------------------------------ B3 · Señal (engine/signal)
struct FilteredSample {
    double t_song = 0;               // tiempo de canción (s)
    JD_Limb limb = JD_LIMB_NONE;
    std::array<float, 3> accel{};    // m/s^2 filtrado, en el marco del sensor
    std::array<float, 3> lin_accel{};// aceleración sin gravedad (marco mundo), m/s^2
    std::array<float, 3> gyro{};     // rad/s filtrado
    std::array<float, 4> quat{1, 0, 0, 0}; // orientación estimada (w,x,y,z), mundo<-sensor
};

class ISignalProcessor {
public:
    virtual ~ISignalProcessor() = default;
    // Convierte muestras brutas a filtradas usando la relación dada (song = t_ns*1e-9 + map).
    virtual void process(const JD_MotionSample* in, int n, double mono_to_song_s,
                         std::vector<FilteredSample>& out) = 0;
    virtual void reset() = 0;
};
std::unique_ptr<ISignalProcessor> make_signal_processor();

// ------------------------------------------------------------------ B4 · Rasgos (engine/features)
struct FeatureFrame {             // rasgos de UNA extremidad en una ventana corta (hop fijo)
    double t_song = 0;            // centro de la ventana
    JD_Limb limb = JD_LIMB_NONE;
    float energy = 0;             // RMS de |lin_accel| en la ventana
    float peak = 0;               // máximo de |lin_accel| en la ventana
    bool  is_peak = false;        // true si esta ventana contiene un pico local destacado
    double peak_t_song = 0;       // instante exacto del pico si is_peak
    std::array<float, 3> dir{};   // dirección media normalizada de lin_accel (mundo)
    std::array<float, 4> quat{1, 0, 0, 0};
};

class IFeatureExtractor {
public:
    virtual ~IFeatureExtractor() = default;
    virtual void push(const std::vector<FilteredSample>& in, std::vector<FeatureFrame>& out) = 0;
    virtual void reset() = 0;
};
std::unique_ptr<IFeatureExtractor> make_feature_extractor();

// ------------------------------------------------------------------ B6 · Coreografía (engine/choreo)
struct Step {
    double t = 0, dur = 0;
    std::vector<JD_Limb> limbs;
    std::string icon;
    enum class Kind { Accent, Move } kind = Kind::Move;
};

struct Choreo {
    std::string title, artist, audio, video;  // video vacío si no hay
    double bpm = 0, offset_s = 0, duration_s = 0;
    std::vector<Step> steps;
    std::string ref_imu;                       // ruta relativa, vacía si no hay
};

// Devuelve false y rellena err ("steps[3].t: ...") si el archivo no es válido.
bool load_choreo(const std::string& choreo_json_path, Choreo& out, std::string& err);
bool validate_choreo(const Choreo& c, std::string& err);

// ------------------------------------------------------------------ B7 · Sesiones (engine/session)
struct SessionDevice { uint32_t source_id = 0; std::string name, kind; JD_Limb limb = JD_LIMB_NONE; };
struct Session {
    std::string time_base = "song", song_id, recorded_at, note;
    std::vector<SessionDevice> devices;
    std::vector<JD_MotionSample> samples;   // t_ns = round(t * 1e9)
};
bool load_session(const std::string& path, Session& out, std::string& err);
bool save_session(const std::string& path, const Session& s, std::string& err);

// ------------------------------------------------------------------ B5 · Juez (engine/judge)
class IJudge {
public:
    virtual ~IJudge() = default;
    // reference_frames: rasgos de la sesión de referencia (vacío si no hay).
    virtual void load(const Choreo& c, const std::vector<FeatureFrame>& reference_frames,
                      const JD_JudgeConfig& cfg) = 0;
    virtual void push(const std::vector<FeatureFrame>& player_frames) = 0;
    // Emite resultados de pasos cuya ventana (t + dur + good_window) ya pasó song_time.
    virtual void update(double song_time_s, std::vector<JD_StepResult>& out) = 0;
    virtual void reset() = 0;
};
std::unique_ptr<IJudge> make_judge();

// Utilidad común: nombre JSON <-> JD_Limb ("left_hand"...). Implementada en engine/common.
const char* limb_to_name(JD_Limb l);
JD_Limb     limb_from_name(const std::string& s);   // JD_LIMB_NONE si no existe

} // namespace jd
