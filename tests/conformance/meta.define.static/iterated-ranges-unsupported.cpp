// [meta.reflection.array]: reflect_constant_array of an iterated range
// (spec/reflection.md §3.5) whose elements are `bool`s from a
// `vector<bool>` or arrays. kiln copies an iterated range into a
// `std::vector` of its value type, which neither holds a `bool` as a
// contiguous element nor holds an array, so both calls are "undefined
// function" errors. Found running GCC's C++ testsuite.
//@ args: -stdlib=libstdc++ -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(kiln): WP-1489, an iterated range of bool or of arrays is not copied into a vector
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

consteval info bools() { return REFLECT_ARRAY(std::vector<bool>{true, false, true}); }
static_assert(type_of(bools()) == ^^const bool[3]);

constexpr int m[2][2]{{1, 2}, {3, 4}};
constexpr info rows = REFLECT_ARRAY(std::span(m) | std::views::reverse);
static_assert(type_of(rows) == ^^const int[2][2]);
