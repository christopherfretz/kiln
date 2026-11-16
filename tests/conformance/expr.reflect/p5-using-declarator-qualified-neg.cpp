// [expr.reflect]/5.1: a qualified reflection-name that lookup finds through
// a using-declarator is ill-formed, also when it names a type or a
// function (the reflection-name form is decided before the type-id form,
// /5 before /6); [temp.res]/5: a dependent `^^X::x` that names a type at
// instantiation needed `typename`. GCC 16.1 rejects the first and the
// last (g++.dg/reflect p2996-14), but accepts the other two; the text
// decides.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ diag: 20:26 error "cannot take the reflection of 'S', which names a declaration introduced by a using-declaration"
//@ diag: 21:26 error "cannot take the reflection of 'U', which names a declaration introduced by a using-declaration"
//@ diag: 22:26 error "cannot take the reflection of 'h', which names a declaration introduced by a using-declaration"
//@ diag: 24:45 error "missing 'typename' prior to dependent type name 'W::x'"
//@ diag: 25:21 note "in instantiation of variable template specialization 'rx<W>' requested here"
#include <meta>
struct A { struct S {}; using U = int; void h(); };
struct B : A { using A::S; using A::U; using A::h; };
constexpr auto r1 = ^^B::S;
constexpr auto r2 = ^^B::U;
constexpr auto r3 = ^^B::h;
struct W { struct x {}; };
template <class X> constexpr auto rx = ^^X::x;
constexpr auto r4 = rx<W>;
