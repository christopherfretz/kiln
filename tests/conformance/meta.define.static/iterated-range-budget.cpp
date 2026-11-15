// [meta.reflection.array] (spec/reflection.md §3.5 "Iterating"): a
// 10,000-element `views::transform` range is iterated once into a
// transient vector that is freed before the evaluation ends, so the
// Oven's peak is the vector's (about 40 KB of elements plus growth),
// not the reflected array's copies.
//@ args: -stdlib=libstdc++ -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oven: 27 peak_bytes<=131072 allocs<=24
#include <cstddef>
#include <ranges>
#ifdef __KILN__
#include "kiln_meta.h"
#define REFLECT_ARRAY(...) std::meta::__kiln_reflect_constant_array(__VA_ARGS__)
#else
#include <meta>
#define REFLECT_ARRAY(...) std::meta::reflect_constant_array(__VA_ARGS__)
#endif
using namespace std::meta;

consteval std::size_t ten_thousand() {
  info r = REFLECT_ARRAY(std::views::iota(0, 10000) |
                         std::views::transform([](int i) { return i * 3; }));
  return (type_of(r) == ^^const int[10000]) * 10000 + extract<const int*>(r)[9999];
}
static_assert(ten_thousand() == 10000 + 29997);
