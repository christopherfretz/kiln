// [dcl.type.splice]/1, [temp.res.general]/4.3 (P2996R13): in the
// parameter-declaration-clause of a constructor declared in its class, a
// splice-type-specifier starts a parameter's decl-specifier-seq, so
// `B([: ^^int :] p)` declares a constructor (a splice is never the
// declarator-id of a parenthesized declarator); likewise after a
// qualified constructor name outside the class, for a constructor
// template's dependent splice, and with a `typename` splice. Found
// running GCC's C++ testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 does not take `B::B([: ^^long :] p)` at namespace scope for a constructor declarator
//@ expect: compile-ok
//@ strict-diags
#include <meta>
struct A {
  int a;
  consteval A(int p) : a(p) {}
};
constexpr auto ra = ^^A;
struct B : A {
  using [:ra:]::A;
  consteval B([:^^int:] p, [:^^int:] q) : A(p * q) {}
  consteval B([:^^long:] p);
  explicit consteval B(typename [:^^char:] c) : A(c) {}
};
consteval B::B([:^^long:] p) : A(int(p) + 1) {}
template <class T> struct TC {
  T v;
  constexpr TC([:^^T:] t) : v(t) {}
};
static_assert(B(6, 7).a == 42 && B(4L).a == 5 && B('a').a == 'a' && B(3).a == 3);
static_assert(TC<int>(9).v == 9);
