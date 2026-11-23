// [temp.variadic]/5, [expr.reflect]: a reflect-expression whose
// id-expression names a pack (a function parameter pack, a structured
// binding pack, P1061) is the pattern of a pack expansion, each element
// reflecting one element of the pack; extract of each reads it within
// the evaluation that began its lifetime ([meta.reflection.extract]).
// GCC 16.1 agrees. Reduced from real code.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ xfail(kiln): WP-2190 follow-up, ^^ of a value pack element is not an unexpanded pack ("pack expansion does not contain any unexpanded parameter packs")
#include <meta>

struct P { int a; int b; };

template <class T> consteval int bindings() {
  auto [...xs] = T{1, 2};
  return (std::meta::extract<int>(^^xs) + ...);
}
static_assert(bindings<P>() == 3);

template <class... T> consteval int params(T... vs) {
  return (std::meta::extract<int>(^^vs) + ...);
}
static_assert(params(1, 2) == 3);
