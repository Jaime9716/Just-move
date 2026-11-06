// engine/common/json.h — JSON mínimo sin dependencias (Fase 0, arquitecto).
// Suficiente para choreo.json y sesiones JSONL. No es un parser de propósito general rápido.
#pragma once

#include <cstdint>
#include <map>
#include <string>
#include <vector>

namespace jd::json {

class Value {
public:
    enum class Type { Null, Bool, Number, String, Array, Object };

    Value() = default;
    static Value null() { return Value(); }
    static Value boolean(bool b);
    static Value number(double d);
    static Value string(std::string s);
    static Value array();
    static Value object();

    Type type() const { return type_; }
    bool is_null() const { return type_ == Type::Null; }
    bool is_bool() const { return type_ == Type::Bool; }
    bool is_number() const { return type_ == Type::Number; }
    bool is_string() const { return type_ == Type::String; }
    bool is_array() const { return type_ == Type::Array; }
    bool is_object() const { return type_ == Type::Object; }

    bool as_bool(bool def = false) const { return is_bool() ? b_ : def; }
    double as_number(double def = 0) const { return is_number() ? n_ : def; }
    const std::string& as_string() const { return s_; }

    // Array
    const std::vector<Value>& items() const { return a_; }
    std::vector<Value>& items() { return a_; }
    size_t size() const { return is_array() ? a_.size() : (is_object() ? o_.size() : 0); }
    const Value& operator[](size_t i) const;
    void push_back(Value v) { a_.push_back(std::move(v)); }

    // Object (orden de inserción conservado para escribir de forma estable)
    bool has(const std::string& k) const;
    const Value& operator[](const std::string& k) const;  // Null si no existe
    void set(const std::string& k, Value v);
    const std::vector<std::pair<std::string, Value>>& members() const { return o_; }

    // Serialización compacta (una línea), números con hasta 9 cifras significativas.
    std::string dump() const;

private:
    Type type_ = Type::Null;
    bool b_ = false;
    double n_ = 0;
    std::string s_;
    std::vector<Value> a_;
    std::vector<std::pair<std::string, Value>> o_;
};

// Devuelve false y un error con línea:columna si el texto no es JSON válido.
bool parse(const std::string& text, Value& out, std::string& err);
bool parse_file(const std::string& path, Value& out, std::string& err);

} // namespace jd::json
