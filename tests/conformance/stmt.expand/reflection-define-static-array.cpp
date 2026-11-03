// [stmt.expand] over P3491R3's define_static_array/define_static_string
// (spec/reflection-values.md §4.5, WP-479): the iterating form expands
// over the frozen array the span designates, whether the call is the
// expansion-initializer itself, a constexpr span variable, a span of
// class values or a string; and an expansion can feed reflect_constant,
// the define_static_* results being usable in every constant context.
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"
struct P { int x, y; };
constexpr int v[] = {1, 2, 3, 4};
constexpr P ps[] = {{1, 2}, {3, 4}};

consteval int direct() {
  int s = 0;
  template for (constexpr int x : std::define_static_array(v)) {
    static_assert(x >= 1 && x <= 4);
    s += x;
  }
  return s;
}
static_assert(direct() == 10);

// The span form: a constexpr span variable as the expansion-initializer.
constexpr std::span<const int> sp = std::define_static_array(v);
consteval int via_span() {
  int s = 0;
  template for (constexpr int x : sp) { s = s * 10 + x; }
  return s;
}
static_assert(via_span() == 1234);

// Class elements: each x is a constexpr P.
consteval int classes() {
  int s = 0;
  template for (constexpr P p : std::define_static_array(ps)) {
    static_assert(p.y == p.x + 1);
    s += p.x * p.y;
  }
  return s;
}
static_assert(classes() == 14);

// A string's characters, terminator included (the range is the array).
consteval int chars() {
  int n = 0;
  template for (constexpr char c : std::define_static_array("abc")) {
    if constexpr (c != '\0') ++n;
  }
  return n;
}
static_assert(chars() == 3);

// Each element as a value reflection: equal values, one reflection.
constexpr int dup[] = {7, 7, 8};
consteval int reflections() {
  int same = 0;
  template for (constexpr int x : std::define_static_array(dup)) {
    if (std::meta::reflect_constant(x) == std::meta::reflect_constant(7)) ++same;
  }
  return same;
}
static_assert(reflections() == 2);

// An empty range expands to nothing.
constexpr int none[1] = {};
consteval int empty() {
  int n = 0;
  template for (constexpr int x : std::define_static_array(std::span<const int>(none, 0))) {
    (void)x;
    ++n;
  }
  return n;
}
static_assert(empty() == 0);

// Outside a consteval function: a namespace-scope expansion in a
// function body, each element usable as a template argument.
template <int N> constexpr int sq = N * N;
int f() {
  int s = 0;
  template for (constexpr int x : std::define_static_array(v)) { s += sq<x>; }
  return s;
}
