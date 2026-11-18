// [expr.const.reflect] (CWG3162), spec/reflection.md §3.6 and §6.2: an
// answer about a class completed by define_aggregate during the running
// evaluation depends on whether a nested evaluation asks it, so no call
// that asks one is memoized in that evaluation (size(^^S) twice in the
// consteval block: no memo hit), neither the outer answer nor the nested
// one. Once that evaluation ended the class is complete everywhere and
// the same Stable call is memoized again (the second static_assert).
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ oven: 24 memo_hits<=0
//@ oven: 33 memo_hits>=1
#include <meta>
struct S;
consteval std::size_t size(std::meta::info r) { return std::meta::size_of(r); }
consteval std::size_t size_if_complete(std::meta::info r) {
  return std::meta::is_complete_type(r) ? size(r) : 0;
}
consteval std::size_t nested() {
  constexpr std::size_t n = size_if_complete(^^S);
  return n;
}
consteval {
  std::meta::define_aggregate(^^S, {});
  if (size(^^S) != 1) throw 1;
  if (nested() != 0) throw 2;
  if (size(^^S) != 1) throw 3;
  if (nested() != 0) throw 4;
  if (size_if_complete(^^S) != 1) throw 5;
}
static_assert(size(^^S) == 1);
static_assert(size(^^S) == 1);
