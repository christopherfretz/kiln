// [temp.dep.namespace]/1, [namespace.alias]/2-3 (P2996R13): a namespace
// alias of a dependent splice-specifier at block scope is a dependent
// namespace alias; names qualified by it are dependent and are looked up
// in the namespace the substituted splice designates, in each
// instantiation: a variable and a function (a nested namespace's member
// is in dependent-namespace-alias-typename.cpp), in
// a static member function template of a class template and a generic
// lambda, through a namespace alias reflected as the argument. WP-1613.
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: compile-ok
namespace NS {
  constexpr int v = 3;
  constexpr int f() { return 10; }
}
namespace NSA = NS;
template <decltype(^^int) R> constexpr int fn() {
  namespace Alias = [:R:];
  return Alias::v + Alias::f();
}
template <class X> struct C {
  template <decltype(^^int) R> static constexpr int m() {
    namespace A = [:R:];
    return A::v + sizeof(X);
  }
};
constexpr auto lam = []<decltype(^^int) R>() {
  namespace A = [:R:];
  return A::f();
};
static_assert(fn<^^NS>() == 13);
static_assert(fn<^^NSA>() == 13);
static_assert(C<char>::m<^^NS>() == 4);
static_assert(lam.template operator()<^^NS>() == 10);
