// [expr.prim.id.general]/4, [temp.inst]/5: a qualified name, an address,
// and a member access naming a member function with a placeholder return
// type whose constraints are not satisfied: the constraints are checked
// first, so the return type is never deduced (WP-1497).
//@ expect: compile-ok
//@ std: c++20
template <class T> struct A {
  static auto r() requires false { return T::fail; }
  auto m() requires false { return T::fail; }
  static auto s() requires (sizeof(T) > 100) { return T::fail; }
  static auto ok() requires true { return 1; }
};
template <class T> concept C1 = requires { A<T>::r(); };
template <class T> concept C2 = requires (A<T> a) { a.m(); };
template <class T> concept C3 = requires { &A<T>::r; };
template <class T> concept C4 = requires { A<T>::s(); };
template <class T> concept C5 = requires { A<T>::ok(); };
static_assert(!C1<int>);
static_assert(!C2<int>);
static_assert(!C3<int>);
static_assert(!C4<int>);
static_assert(C5<int>);
template <class T> constexpr int f() { if constexpr (requires { A<T>::r(); }) return 1; else return 2; }
static_assert(f<int>() == 2);
