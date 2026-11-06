// engine/common/common.cpp — utilidades compartidas (Fase 0).
#include <chrono>

#include "../../contracts/clock.h"
#include "../../contracts/engine_internal.h"

extern "C" int64_t jd_now_ns(void) {
    using namespace std::chrono;
    return duration_cast<nanoseconds>(steady_clock::now().time_since_epoch()).count();
}

namespace jd {

const char* limb_to_name(JD_Limb l) {
    switch (l) {
        case JD_LIMB_LEFT_HAND: return "left_hand";
        case JD_LIMB_RIGHT_HAND: return "right_hand";
        case JD_LIMB_LEFT_FOOT: return "left_foot";
        case JD_LIMB_RIGHT_FOOT: return "right_foot";
        default: return "none";
    }
}

JD_Limb limb_from_name(const std::string& s) {
    if (s == "left_hand") return JD_LIMB_LEFT_HAND;
    if (s == "right_hand") return JD_LIMB_RIGHT_HAND;
    if (s == "left_foot") return JD_LIMB_LEFT_FOOT;
    if (s == "right_foot") return JD_LIMB_RIGHT_FOOT;
    return JD_LIMB_NONE;
}

} // namespace jd
