#include "json.h"

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <fstream>
#include <sstream>

namespace jd::json {

static const Value kNull;

Value Value::boolean(bool b) { Value v; v.type_ = Type::Bool; v.b_ = b; return v; }
Value Value::number(double d) { Value v; v.type_ = Type::Number; v.n_ = d; return v; }
Value Value::string(std::string s) { Value v; v.type_ = Type::String; v.s_ = std::move(s); return v; }
Value Value::array() { Value v; v.type_ = Type::Array; return v; }
Value Value::object() { Value v; v.type_ = Type::Object; return v; }

const Value& Value::operator[](size_t i) const {
    return (is_array() && i < a_.size()) ? a_[i] : kNull;
}
bool Value::has(const std::string& k) const {
    if (!is_object()) return false;
    for (auto& m : o_) if (m.first == k) return true;
    return false;
}
const Value& Value::operator[](const std::string& k) const {
    if (is_object()) for (auto& m : o_) if (m.first == k) return m.second;
    return kNull;
}
void Value::set(const std::string& k, Value v) {
    if (type_ != Type::Object) { *this = object(); }
    for (auto& m : o_) if (m.first == k) { m.second = std::move(v); return; }
    o_.emplace_back(k, std::move(v));
}

static void dump_string(const std::string& s, std::string& out) {
    out += '"';
    for (unsigned char c : s) {
        switch (c) {
            case '"': out += "\\\""; break;
            case '\\': out += "\\\\"; break;
            case '\n': out += "\\n"; break;
            case '\r': out += "\\r"; break;
            case '\t': out += "\\t"; break;
            default:
                if (c < 0x20) { char buf[8]; std::snprintf(buf, sizeof buf, "\\u%04x", c); out += buf; }
                else out += static_cast<char>(c);
        }
    }
    out += '"';
}

static void dump_value(const Value& v, std::string& out) {
    switch (v.type()) {
        case Value::Type::Null: out += "null"; break;
        case Value::Type::Bool: out += v.as_bool() ? "true" : "false"; break;
        case Value::Type::Number: {
            double d = v.as_number();
            if (!std::isfinite(d)) { out += "null"; break; }
            char buf[32];
            std::snprintf(buf, sizeof buf, "%.9g", d);
            out += buf;
            break;
        }
        case Value::Type::String: dump_string(v.as_string(), out); break;
        case Value::Type::Array: {
            out += '[';
            bool first = true;
            for (auto& it : v.items()) { if (!first) out += ','; first = false; dump_value(it, out); }
            out += ']';
            break;
        }
        case Value::Type::Object: {
            out += '{';
            bool first = true;
            for (auto& m : v.members()) {
                if (!first) out += ',';
                first = false;
                dump_string(m.first, out);
                out += ':';
                dump_value(m.second, out);
            }
            out += '}';
            break;
        }
    }
}

std::string Value::dump() const { std::string s; dump_value(*this, s); return s; }

namespace {
struct Parser {
    const std::string& t;
    size_t i = 0;
    std::string err;

    explicit Parser(const std::string& text) : t(text) {}

    bool fail(const char* msg) {
        size_t line = 1, col = 1;
        for (size_t k = 0; k < i && k < t.size(); ++k) { if (t[k] == '\n') { ++line; col = 1; } else ++col; }
        err = std::to_string(line) + ":" + std::to_string(col) + ": " + msg;
        return false;
    }
    void ws() { while (i < t.size() && (t[i] == ' ' || t[i] == '\t' || t[i] == '\n' || t[i] == '\r')) ++i; }
    bool lit(const char* s) {
        size_t n = std::char_traits<char>::length(s);
        if (t.compare(i, n, s) == 0) { i += n; return true; }
        return false;
    }
    static void utf8(uint32_t cp, std::string& o) {
        if (cp < 0x80) o += char(cp);
        else if (cp < 0x800) { o += char(0xC0 | (cp >> 6)); o += char(0x80 | (cp & 0x3F)); }
        else if (cp < 0x10000) { o += char(0xE0 | (cp >> 12)); o += char(0x80 | ((cp >> 6) & 0x3F)); o += char(0x80 | (cp & 0x3F)); }
        else { o += char(0xF0 | (cp >> 18)); o += char(0x80 | ((cp >> 12) & 0x3F)); o += char(0x80 | ((cp >> 6) & 0x3F)); o += char(0x80 | (cp & 0x3F)); }
    }
    bool hex4(uint32_t& v) {
        if (i + 4 > t.size()) return fail("escape \\u incompleto");
        v = 0;
        for (int k = 0; k < 4; ++k) {
            char c = t[i++];
            v <<= 4;
            if (c >= '0' && c <= '9') v |= c - '0';
            else if (c >= 'a' && c <= 'f') v |= c - 'a' + 10;
            else if (c >= 'A' && c <= 'F') v |= c - 'A' + 10;
            else return fail("escape \\u no hexadecimal");
        }
        return true;
    }
    bool str(std::string& o) {
        ++i;  // "
        while (i < t.size()) {
            char c = t[i++];
            if (c == '"') return true;
            if (c == '\\') {
                if (i >= t.size()) break;
                char e = t[i++];
                switch (e) {
                    case '"': o += '"'; break; case '\\': o += '\\'; break; case '/': o += '/'; break;
                    case 'b': o += '\b'; break; case 'f': o += '\f'; break; case 'n': o += '\n'; break;
                    case 'r': o += '\r'; break; case 't': o += '\t'; break;
                    case 'u': {
                        uint32_t cp;
                        if (!hex4(cp)) return false;
                        if (cp >= 0xD800 && cp <= 0xDBFF && t.compare(i, 2, "\\u") == 0) {
                            i += 2; uint32_t lo; if (!hex4(lo)) return false;
                            cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                        }
                        utf8(cp, o);
                        break;
                    }
                    default: return fail("escape desconocido");
                }
            } else {
                o += c;
            }
        }
        return fail("cadena sin cerrar");
    }
    bool val(Value& v, int depth) {
        if (depth > 200) return fail("anidamiento excesivo");
        ws();
        if (i >= t.size()) return fail("se esperaba un valor");
        char c = t[i];
        if (c == '{') {
            ++i; v = Value::object(); ws();
            if (i < t.size() && t[i] == '}') { ++i; return true; }
            while (true) {
                ws();
                if (i >= t.size() || t[i] != '"') return fail("se esperaba una clave entre comillas");
                std::string k; if (!str(k)) return false;
                ws();
                if (i >= t.size() || t[i] != ':') return fail("se esperaba ':'");
                ++i;
                Value child; if (!val(child, depth + 1)) return false;
                v.set(k, std::move(child));
                ws();
                if (i < t.size() && t[i] == ',') { ++i; continue; }
                if (i < t.size() && t[i] == '}') { ++i; return true; }
                return fail("se esperaba ',' o '}'");
            }
        }
        if (c == '[') {
            ++i; v = Value::array(); ws();
            if (i < t.size() && t[i] == ']') { ++i; return true; }
            while (true) {
                Value child; if (!val(child, depth + 1)) return false;
                v.push_back(std::move(child));
                ws();
                if (i < t.size() && t[i] == ',') { ++i; continue; }
                if (i < t.size() && t[i] == ']') { ++i; return true; }
                return fail("se esperaba ',' o ']'");
            }
        }
        if (c == '"') { std::string s; if (!str(s)) return false; v = Value::string(std::move(s)); return true; }
        if (lit("true")) { v = Value::boolean(true); return true; }
        if (lit("false")) { v = Value::boolean(false); return true; }
        if (lit("null")) { v = Value::null(); return true; }
        if (c == '-' || (c >= '0' && c <= '9')) {
            const char* start = t.c_str() + i;
            char* end = nullptr;
            double d = std::strtod(start, &end);
            if (end == start) return fail("número no válido");
            i += size_t(end - start);
            v = Value::number(d);
            return true;
        }
        return fail("carácter inesperado");
    }
};
} // namespace

bool parse(const std::string& text, Value& out, std::string& err) {
    Parser p(text);
    Value v;
    if (!p.val(v, 0)) { err = p.err; return false; }
    p.ws();
    if (p.i != text.size()) { p.fail("contenido extra tras el JSON"); err = p.err; return false; }
    out = std::move(v);
    return true;
}

bool parse_file(const std::string& path, Value& out, std::string& err) {
    std::ifstream f(path, std::ios::binary);
    if (!f) { err = "no se puede abrir " + path; return false; }
    std::stringstream ss; ss << f.rdbuf();
    if (!parse(ss.str(), out, err)) { err = path + ":" + err; return false; }
    return true;
}

} // namespace jd::json
