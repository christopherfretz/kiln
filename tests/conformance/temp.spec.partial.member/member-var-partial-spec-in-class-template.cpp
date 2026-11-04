// A partial specialization of a member variable template declared in a
// class template, in class or out of line, is redeclared for the member
// template of each specialization of the enclosing class
// ([temp.spec.partial.member]/1-2; WP-525, the shape of libstdc++ 16
// <optional> `__not_constructing_bool_from_optional<_From, bool>`): it is
// matched with the enclosing arguments substituted, and the matched
// specialization's type and initializer come from the original partial
// specialization (clang `InstantiateVarTemplatePartialSpecialization`).
// Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
template <class T> struct O {
  template <class F, class U = T> static constexpr bool nc = true;
  template <class F> static constexpr bool nc<F, bool> = false;
  template <class F> static constexpr int sz = 0;
  template <class F> static constexpr int sz<F*> = sizeof(F) + sizeof(T);
};
static_assert(O<int>::nc<int> && !O<bool>::nc<int> && !O<int>::nc<int, bool>);
static_assert(O<char>::sz<int> == 0 && O<char>::sz<int*> == sizeof(int) + 1);
template <class T> struct P {
  template <class F> static constexpr int v = 1;
};
template <class T> template <class F> constexpr int P<T>::v<F*> = 2 + sizeof(T);
static_assert(P<char>::v<int> == 1 && P<char>::v<int*> == 3);
template <class T> constexpr bool use() { return O<T>::template nc<T>; }
static_assert(use<int>() && !use<bool>());
