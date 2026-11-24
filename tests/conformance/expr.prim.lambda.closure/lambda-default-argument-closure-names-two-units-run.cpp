// [expr.prim.lambda.closure]/2, [dcl.fct.default]/3, [temp.param]/10: the
// closure types of lambdas in a lambda's default arguments take clang 19's
// and 22's names (spec/abi.md §8.9, WP-2627, WP-2628). A lambda in the
// default argument of a lambda in a function template is instantiated
// with the enclosing lambda and numbered per its new call operator's
// parameter (`Ed_`); a generic lambda's default template argument is
// numbered in its closure type at the use, a <nested-name> naming the
// function (`N2a3UlTyvE_UlvE_E`); a local class's default member
// initializer numbers its closure per the member. Each closure names a
// specialization Inputs/lambda-default-argument-closure-names-defs.cpp
// defines under clang's name, so the program links only if kiln's
// references carry those names.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-default-argument-closure-names-defs.cpp
//@ stdout: 1 2 3
extern "C" int printf(const char *, ...);
template <class F> int tk2(F);
template <class T> int tk();
template <class T> int a2() {
  auto t = [](int u = tk2([] {})) { return u; };
  return t();
}
inline int a3() {
  auto g = []<class T = decltype([] {})>() { return tk<T>(); };
  return g();
}
inline int r() {
  struct L {
    decltype([] {}) m;
    int n = tk2([] { return 0; });
  };
  return L().n;
}
int main() { printf("%d %d %d\n", a2<int>(), a3(), r()); }
