// engine/common/mini_test.h — arnés de tests sin dependencias (Fase 0).
// GoogleTest no está disponible en el entorno de desarrollo; esta cabecera da lo justo.
//
//   #include "mini_test.h"
//   TEST(nombre) { CHECK(1 + 1 == 2); CHECK_NEAR(x, 1.0, 1e-6); }
//   int main() { return jd_test::run_all(); }     // o enlazar con mini_test_main.cpp
//
// Cada ejecutable de test se registra en CTest con add_test().
#pragma once

#include <cmath>
#include <cstdio>
#include <functional>
#include <string>
#include <vector>

namespace jd_test {

struct Case { const char* name; std::function<void()> fn; };
inline std::vector<Case>& registry() { static std::vector<Case> r; return r; }
inline int& failures() { static int f = 0; return f; }
inline bool& current_failed() { static bool b = false; return b; }

struct Registrar {
    Registrar(const char* n, std::function<void()> f) { registry().push_back({n, std::move(f)}); }
};

inline void report(const char* file, int line, const std::string& msg) {
    std::fprintf(stderr, "  FALLO %s:%d: %s\n", file, line, msg.c_str());
    current_failed() = true;
}

inline int run_all(const char* filter = nullptr) {
    int passed = 0, failed = 0;
    for (auto& c : registry()) {
        if (filter && std::string(c.name).find(filter) == std::string::npos) continue;
        current_failed() = false;
        try { c.fn(); } catch (const std::exception& e) { report(__FILE__, __LINE__, std::string("excepción: ") + e.what()); }
        catch (...) { report(__FILE__, __LINE__, "excepción desconocida"); }
        if (current_failed()) { ++failed; std::fprintf(stderr, "[FALLA] %s\n", c.name); }
        else { ++passed; std::printf("[ OK  ] %s\n", c.name); }
    }
    std::printf("%d ok, %d fallan\n", passed, failed);
    return failed == 0 ? 0 : 1;
}

} // namespace jd_test

#define JD_TEST_CAT2(a, b) a##b
#define JD_TEST_CAT(a, b) JD_TEST_CAT2(a, b)
#define TEST(name)                                                                  \
    static void JD_TEST_CAT(jd_test_fn_, name)();                                   \
    static jd_test::Registrar JD_TEST_CAT(jd_test_reg_, name)(#name, JD_TEST_CAT(jd_test_fn_, name)); \
    static void JD_TEST_CAT(jd_test_fn_, name)()

#define CHECK(cond) do { if (!(cond)) jd_test::report(__FILE__, __LINE__, "CHECK(" #cond ")"); } while (0)
#define CHECK_EQ(a, b) do { if (!((a) == (b))) jd_test::report(__FILE__, __LINE__, "CHECK_EQ(" #a ", " #b ")"); } while (0)
#define CHECK_NEAR(a, b, tol) do { double _a = (a), _b = (b); if (!(std::fabs(_a - _b) <= (tol))) \
    jd_test::report(__FILE__, __LINE__, "CHECK_NEAR(" #a ", " #b "): " + std::to_string(_a) + " vs " + std::to_string(_b)); } while (0)
#define REQUIRE(cond) do { if (!(cond)) { jd_test::report(__FILE__, __LINE__, "REQUIRE(" #cond ")"); return; } } while (0)
