// [expr.reflect]/1, [temp.names]/5: after a dependent qualifier,
// `^^X::template B` names the member template B of the type X becomes
// (a class template here). GCC 16.1 rejects it ("parsed as a non-type,
// but instantiation yields a type"); the `template` keyword says the
// name is a template, so the text decides.
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
#include <meta>
struct S {
  template <class T> struct B {};
};
template <class X> constexpr auto rd = ^^X::template B;
static_assert(rd<S> == ^^S::B);
