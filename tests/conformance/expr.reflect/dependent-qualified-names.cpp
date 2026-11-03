// [expr.reflect]/5-7, [temp.dep.expr] (P2996R13): in a template, `^^T::m`,
// `^^typename T::type`, `^^typename T::template X<U>`, `^^ns::X<T>::m` and the
// nested `^^T::template X<U>::m` are dependent reflect-expressions,
// resolved at instantiation to what the qualified-id names there: a
// non-static or static data member, a single member function, a member
// enumerator, a member type, a member class template specialization, a
// member of one. Each round-trips with `^^` of the non-dependent name and
// the queries agree (spec/reflection-repr.md §2).
// Compared with GCC 16.1 (spec/harness.md "GCC oracle").
//@ expect: compile-ok
//@ oracle: gcc
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#if __has_include(<meta>)
#include <meta>
#else
#include "kiln_meta_queries.h"
#endif
using std::meta::info;
namespace ns {
struct S {
  int x;
  static int s;
  void f();
  using type = long;
  enum E { e0, e1 };
  template <class U> struct inner { static constexpr int y = sizeof(U); };
};
template <class T> struct W { T w; };
}  // namespace ns
using namespace ns;

template <class T> consteval info rx() { return ^^T::x; }
template <class T> consteval info rs() { return ^^T::s; }
template <class T> consteval info rf() { return ^^T::f; }
template <class T> consteval info re() { return ^^T::e1; }
template <class T> consteval info rtype() { return ^^typename T::type; }
template <class T, class U> consteval info rinner() { return ^^typename T::template inner<U>; }
template <class T, class U> consteval info ry() { return ^^T::template inner<U>::y; }
template <class T> consteval info rns() { return ^^ns::W<T>::w; }

static_assert(rx<S>() == ^^S::x);
static_assert(rs<S>() == ^^S::s);
static_assert(rf<S>() == ^^S::f);
static_assert(re<S>() == ^^S::e1);
static_assert(rtype<S>() == ^^S::type);
static_assert(rinner<S, int>() == ^^S::inner<int>);
static_assert(ry<S, int>() == ^^S::inner<int>::y);
static_assert(ry<S, char>() != ry<S, int>());
static_assert(rns<int>() == ^^ns::W<int>::w);

static_assert(std::meta::is_nonstatic_data_member(rx<S>()));
static_assert(std::meta::type_of(rx<S>()) == ^^int);
static_assert(std::meta::identifier_of(rx<S>()) == "x");
static_assert(std::meta::parent_of(rx<S>()) == ^^S);
static_assert(std::meta::is_static_member(rs<S>()) && std::meta::is_variable(rs<S>()));
static_assert(std::meta::is_function(rf<S>()) && std::meta::identifier_of(rf<S>()) == "f");
static_assert(std::meta::is_enumerator(re<S>()) && std::meta::identifier_of(re<S>()) == "e1");
static_assert(std::meta::type_of(re<S>()) == ^^S::E);
static_assert(std::meta::is_type(rtype<S>()));
static_assert(std::meta::is_type(rinner<S, int>()));
static_assert(std::meta::parent_of(ry<S, int>()) == ^^S::inner<int>);
static_assert(std::meta::identifier_of(ry<S, int>()) == "y");

// The same template instantiated for another class reflects its members.
struct R { char x; };
template <> struct ns::S::inner<R> { static constexpr int y = 0; };
static_assert(rx<R>() == ^^R::x && rx<R>() != rx<S>());
static_assert(std::meta::type_of(rx<R>()) == ^^char);

// A member of a class template, named through the current instantiation.
template <class T> struct Box {
  T v;
  static consteval info self_v() { return ^^Box::v; }
};
static_assert(Box<int>::self_v() == ^^Box<int>::v);
static_assert(std::meta::type_of(Box<short>::self_v()) == ^^short);
