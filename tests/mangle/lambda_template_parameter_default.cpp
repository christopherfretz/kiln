// Mangling differential: lambdas in default template arguments
// (spec/abi.md §8.15, WP-2628; [temp.param]/10,
// [expr.prim.lambda.closure]/2). The closure type written in a template
// parameter's default argument is never numbered where it is parsed (clang
// 19 and 22 give it no number there, so it takes none of its scope's);
// each use substitutes the default and numbers the new closure in the
// template's context: two defaults of one template take consecutive
// numbers (`_Z3twoITnDaXtlUlvE_EETnDaXtlUlvE0_EEEiv`), a class template's
// default its namespace's. A generic lambda's template parameter default
// is numbered in the lambda's closure type at each use
// (`_ZZ3genvENKUlTnDavE_UlvE_clEv`); that closure type, whose context is
// a class inside a function, mangles as a <nested-name> naming the
// function as a plain <unqualified-name> (`N3genUlTnDavE_UlvE_E`, clang's
// 19 ABI), not a <local-name>.

template <auto F = [] { return 1; }, auto G = [] { return 2; }> int two() { return F() + G(); }
template <class T = decltype([] {}), class U = decltype([] {})> int two_t() { return 3; }
template <auto F = [] { return 4; }> struct C { int m() { return F(); } };
namespace n {
template <auto F = [] { return 5; }> int g() { return F(); }
template <class T = decltype([] {})> struct D { static int v() { return 6; } };
}
struct S {
  template <auto F = [] { return 7; }> static int g() { return F(); }
  template <class T = decltype([] {})> static int h() { return 8; }
};
template <class T> struct Q {
  template <auto F = [] { return T(9); }> int g() { return F(); }
};
inline int gen() {
  auto g2 = []<auto F = [] { return 10; }>() { return F(); };
  auto g3 = []<class T = decltype([] { return 11; })>() { return T()(); };
  auto w = [] { return 12; };
  return g2() + g2() + g3() + w();
}

int use() {
  Q<int> q;
  return two() + two() + two_t() + C<>().m() + n::g() + n::D<>::v() + S::g() + S::h() + q.g()
       + gen();
}
