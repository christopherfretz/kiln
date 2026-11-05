// [temp.res.general]/4-5 (C++20, P0634R3): `typename` is optional for a
// qualified name in a context that can only be a type.
// (A parameter of an unqualified namespace-scope function is not such a
// context, so `f` still spells `typename` there.)
// WP-646 feature probe.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
template <class T> T::type f(typename T::type x) { return x; }
template <class T> struct S {
  using A = T::type;
  T::type member;
  T::type g(T::type);
  static_assert(__is_same(decltype(static_cast<T::type>(0)), A));
};
struct Q { using type = int; };
template <class T> T::type S<T>::g(T::type v) { return v; }
int use() { return f<Q>(1) + S<Q>{}.g(2); }
