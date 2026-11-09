// [temp.dep.constexpr]/2, [expr.const]/3: a reference variable is
// potentially-constant, so one bound to a constant initialized from a
// template parameter, or to another such reference, is value-dependent:
// template-ids and static_asserts naming it wait for instantiation, and the
// templates are never instantiated (WP-958).
//@ std: c++20
//@ expect: compile-ok
template <int n> struct S;
template <int n> void f() {
  const int k = n;
  const int &i = k;
  static_assert(i == 1);
  S<i>::value;
}
template <int n> struct A {
  void g() {
    const int k = n * 2;
    const int &r = k;
    const int &rr = r;
    typename S<rr>::type x;
    static_assert(sizeof(S<rr>) == 3);
  }
};
