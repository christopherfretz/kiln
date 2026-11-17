// [expr.reflect]/7 (P2996R13), spec/reflection-repr.md §2: `^^f<0>` in a
// class template, `f` a member function template whose type depends on
// the class's template parameters, reflects in each specialization of the
// class that specialization's `f<0>` (its type is the substituted one),
// with or without `S::template`, whether `f` is static or not; a
// non-dependent one is unaffected. Found running GCC's C++ testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
//@ strict-diags
#include <meta>
template <class F> struct S {
  template <int I> static constexpr int visit(F &&) { return 40 + I; }
  template <int I> int mem(F) const { return I; }
  template <int I> static int plain(int) { return I; }
  static constexpr std::meta::info a = ^^visit<2>;
  static constexpr std::meta::info b = ^^S::template visit<1>;
  static constexpr std::meta::info c = ^^mem<3>;
  static constexpr std::meta::info d = ^^plain<4>;
  using Ptr = decltype(&visit<0>);
};
struct Plus {};
static_assert(type_of(S<Plus>::a) == ^^int(Plus &&));
static_assert(type_of(S<Plus>::b) == ^^int(Plus &&));
static_assert(type_of(S<long>::c) == ^^int(long) const);
static_assert(type_of(S<long>::d) == ^^int(int));
static_assert(S<Plus>::a == ^^S<Plus>::visit<2> && S<Plus>::a != S<char>::a);
static_assert(std::meta::extract<S<Plus>::Ptr>(S<Plus>::a)(Plus{}) == 42);
static_assert(std::meta::extract<int (*)(Plus &&)>(S<Plus>::b)(Plus{}) == 41);
