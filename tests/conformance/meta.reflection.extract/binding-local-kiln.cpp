// [meta.reflection.extract], [meta.reflection.queries] constant_of: a
// structured binding, local variable or parameter whose lifetime began
// within the constant evaluation is read where its frame holds it, also
// from a function its reflection is passed down to (the reflection
// names the entity; [expr.const]/10 asks only that the lifetime began
// within the evaluation), and in the innermost live activation of a
// recursive function. A tuple-like binding names the object its holding
// variable (a reference, [dcl.struct.bind]/4) refers to; a reference
// local's constant_of is its referent's value. A call given such a
// reflection is not memoized: it reads what the frame holds at that call
// (WP-2074). No GCC oracle: GCC 16.1 rejects extract of another frame's
// variable ("reflection does not represent an annotation or a valid
// argument to a splice-expression"), of a tuple-like binding and
// constant_of of a reference local ("reflect_constant failed"). Reduced
// from real code.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include <meta>
#include <tuple>
#include <utility>

using std::meta::constant_of;
using std::meta::extract;
using std::meta::info;
using std::meta::reflect_constant;

struct P { int a; int b; };

consteval int read(info r) { return extract<int>(r); }
consteval int read_constant(info r) { return extract<int>(constant_of(r)); }
consteval int pass(info r) { return read(r) + 0; }

// Passed down: a local and the bindings of an enclosing frame.
consteval int enclosing() {
  int x = 3;
  auto [a, b] = P{4, 5};
  return read(^^x) * 100 + pass(^^a) * 10 + read_constant(^^b);
}
static_assert(enclosing() == 345);

// The same reflection after the binding changed: read again.
consteval int changed() {
  auto [a, b] = P{1, 2};
  int first = read(^^a);
  a = 5;
  int second = read(^^a);
  b = 7;
  return first * 100 + second * 10 + read_constant(^^b);
}
static_assert(changed() == 157);

// The innermost live activation's binding.
consteval int depth(int n) {
  auto [here, unused] = P{n, 0};
  if (n > 0) return depth(n - 1) + read(^^here);
  return unused;
}
static_assert(depth(3) == 6);

// Tuple-like: std::pair by value, std::tuple through a reference.
consteval int pair() {
  auto [a, b] = std::pair<int, int>{8, 9};
  b = 6;
  return extract<int>(^^a) * 10 + read(^^b);
}
static_assert(pair() == 86);

consteval int tuple() {
  std::tuple<int, long> t{1, 2};
  auto& [a, b] = t;
  std::get<0>(t) = 3;
  return read(^^a) * 10 + static_cast<int>(extract<long>(^^b));
}
static_assert(tuple() == 32);

// A reference local and a reference parameter: their referent's value.
consteval int ref_param(int& r) { return read_constant(^^r); }
consteval int references() {
  int x = 4;
  int& r = x;
  x = 5;
  return read_constant(^^r) * 10 + ref_param(x);
}
static_assert(references() == 55);
