#include "json.h"
#include "mini_test.h"
#include "engine_internal.h"

using namespace jd::json;

TEST(json_roundtrip) {
    Value v; std::string err;
    REQUIRE(parse(R"({"a":[1,2.5,"xé"],"b":true,"c":null})", v, err));
    CHECK(v["a"].size() == 3);
    CHECK_NEAR(v["a"][1].as_number(), 2.5, 1e-12);
    CHECK(v["a"][2].as_string() == "x\xc3\xa9");
    CHECK(v["b"].as_bool());
    CHECK(v["c"].is_null());
    Value w; REQUIRE(parse(v.dump(), w, err));
    CHECK(w.dump() == v.dump());
}
TEST(json_error_has_position) {
    Value v; std::string err;
    CHECK(!parse("{\n \"a\": }", v, err));
    CHECK(err.rfind("2:", 0) == 0);
}
TEST(limb_names) {
    CHECK(jd::limb_from_name("left_foot") == JD_LIMB_LEFT_FOOT);
    CHECK(std::string(jd::limb_to_name(JD_LIMB_RIGHT_HAND)) == "right_hand");
    CHECK(jd::limb_from_name("x") == JD_LIMB_NONE);
}
int main() { return jd_test::run_all(); }
