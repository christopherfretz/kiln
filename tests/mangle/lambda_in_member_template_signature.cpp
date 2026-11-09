// Mangling differential: a lambda in an unevaluated operand of a member
// template's signature, substituted when its class template is
// instantiated ([temp.inst]/3, [expr.prim.lambda.closure]/2). Its closure
// belongs to the non-dependent `S<int>` and is never-dependent (clang
// `LDK_NeverDependent`, spec/sema-lambda.md §9), so `decltype` of it is
// the closure type: `S<int>::mret<int>` is `_ZN1SIiE4mretIiEENS0_UlvE_ET_`
// (spec/abi.md §8.15, WP-1141). A closure's own template parameters keep
// their depth (`UlTL0__E_`); a variable template member's type is not
// re-substituted, so `vt<int>`'s call operator is `_ZNK1SIiEUlvE2_clEv`.
// The two-unit run tests are
// tests/conformance/expr.prim.lambda.closure/lambda-in-signature-member-template-*.
template <class T> struct S {
  template <class U> static auto mret(U) -> decltype([] { return 5; }) { return {}; }
  template <class U> static auto m2(U) -> decltype([] { return 5; }, U{}) { return {}; }
  template <class U> static auto mpar(U, decltype([] { return 6; }) = {}) { return 1; }
  template <class U> static constexpr decltype([] { return 8; }) vt{};
  template <class U> static auto mU(U) -> decltype([] { return U{}; }) { return {}; }
  template <class U> static auto mca(U) -> decltype([] { return sizeof(T); }) { return {}; }
  template <class U> static auto mgen(U) -> decltype([](auto x) { return x; }) { return {}; }
  template <class U> static auto mtpl(U) -> decltype([]<class V>(V x, auto) { return x; }) {
    return {};
  }
  template <class U> struct N {
    template <class V> static auto f(V) -> decltype([] { return 7; }) { return {}; }
  };
};
template <class T> struct C {
  template <class U> static auto mcap(U u) -> decltype([u] { return 5; }) * { return nullptr; }
  template <class U> static auto mref(U u) -> decltype([&u] { return 5; }) * { return nullptr; }
};
template <class A> struct O {
  template <class T> struct S2 {
    template <class U> static auto f(U) -> decltype([] { return 9; }) { return {}; }
  };
};
int use() {
  return S<int>::mret(1)() + S<int>::m2(1) + S<int>::mpar(1) + S<int>::vt<int>()
       + S<int>::mU(1)() + S<int>::mca(1)() + S<int>::mgen(1)(1) + S<int>::mtpl(1)(1, 2)
       + S<int>::N<int>::f(1)() + !C<int>::mcap(1) + !C<int>::mref(1) + O<int>::S2<int>::f(1)();
}
