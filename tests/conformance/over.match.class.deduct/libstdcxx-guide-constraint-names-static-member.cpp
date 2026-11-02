// [over.match.class.deduct]/1.1, /1.2, [temp.constr.decl]/3: the implicit
// deduction guide of a constructor (template) carries the constructor's
// requires-clause, whose unqualified names of the class's static member
// function templates (`ok<U...>()`) name those members, found in the
// deduced specialization when the guide's constraints are checked, not
// looked up again from the guide's namespace scope (libstdc++ tuple:980
// `__disambiguating_constraint`, reached by `views::single(tuple{})` in
// ranges:8829; WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 0 2 1
extern "C" int printf(const char *, ...);
template<class... E> struct T {
  template<class... U> static consteval bool ok() {
    return sizeof...(U) == sizeof...(E);
  }
  int n = -1;
  constexpr T() requires (ok<>()) : n(0) {}
  template<class... U> requires (ok<U...>()) constexpr T(U...) : n(sizeof...(U)) {}
};
template<class E> struct One {
  static consteval bool fits() { return sizeof(E) <= sizeof(long long); }
  int n;
  constexpr One(E) requires (fits()) : n(1) {}
};
template<class... A> int make(A... a) {
  if constexpr (sizeof...(A) == 0)
    return T{}.n;
  else
    return T<A...>{a...}.n;
}
int main() {
  One o{'c'};
  printf("%d %d %d\n", make(), make(1, 2), o.n);
}
