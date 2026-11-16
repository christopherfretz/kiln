// [meta.reflection.array], [meta.define.static]: reflect_constant_array
// and define_static_array of iterated ranges (spec/reflection.md §3.5)
// whose range_value_t is `bool` or an array type. Each element is the
// value type initialized from `*it` (a `vector<bool>::reference` gives a
// `bool`); an array element keeps its type, so the result is an array of
// arrays. The range is traversed once (an input view of `bool`s works).
// Found running GCC's C++ testsuite.
//@ args: -stdlib=libstdc++ -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#include <ranges>
#include <span>
#include <vector>
#ifdef __KILN__
#include "kiln_meta.h"
#define REFLECT_ARRAY(...) std::meta::__kiln_reflect_constant_array(__VA_ARGS__)
#else
#include <meta>
#define REFLECT_ARRAY(...) std::meta::reflect_constant_array(__VA_ARGS__)
#endif
using namespace std::meta;

consteval info bools() { return REFLECT_ARRAY(std::vector<bool>{true, false, true, true}); }
static_assert(type_of(bools()) == ^^const bool[4]);
constexpr const bool* b = extract<const bool*>(bools());
static_assert(b[0] && !b[1] && b[2] && b[3]);

// An input view of bools made from ints.
consteval info odd() {
  return REFLECT_ARRAY(std::views::iota(0, 5) |
                       std::views::transform([](int i) { return i % 2 == 1; }));
}
static_assert(type_of(odd()) == ^^const bool[5]);
static_assert(!extract<const bool*>(odd())[0] && extract<const bool*>(odd())[3]);

constexpr int m[3][2]{{1, 2}, {3, 4}, {5, 6}};
constexpr info rows = REFLECT_ARRAY(std::span(m) | std::views::reverse);
static_assert(type_of(rows) == ^^const int[3][2]);
constexpr auto r = extract<const int(*)[2]>(rows);
static_assert(r[0][0] == 5 && r[0][1] == 6 && r[2][0] == 1 && r[2][1] == 2);

// Arrays of arrays of bool.
constexpr bool bb[2][2][1]{{{true}, {false}}, {{false}, {true}}};
constexpr info deep = REFLECT_ARRAY(std::span(bb) | std::views::reverse);
static_assert(type_of(deep) == ^^const bool[2][2][1]);
static_assert(!extract<const bool(*)[2][1]>(deep)[0][0][0]);
static_assert(extract<const bool(*)[2][1]>(deep)[1][0][0]);

// Equal contents are one object.
static_assert(REFLECT_ARRAY(std::vector<bool>{true, false, true, true}) == bools());

static_assert(std::define_static_array(std::vector<bool>{false, true}).size() == 2);
static_assert(std::define_static_array(std::vector<bool>{false, true})[1]);
