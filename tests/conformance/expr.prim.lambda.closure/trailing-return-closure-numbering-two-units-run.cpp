// [expr.prim.lambda.closure]/2: a lambda-expression in another lambda's
// trailing return type takes no number in the enclosing inline function
// (clang 19 and 22): `w` below is that function's second `UlvE` closure,
// `_ZZ1svEUlvE0_`. Inputs/trailing-return-closure-numbering-defs.cpp
// defines the specialization `w` names under clang's name, so the program
// links only if kiln's reference carries it.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/trailing-return-closure-numbering-defs.cpp
//@ stdout: 5
//@ xfail(kiln): WP-2800 follow-up, a lambda in a lambda's trailing return type is numbered in the enclosing inline function
extern "C" int printf(const char *, ...);
template <class F> int tk2(F);
inline int s() {
  auto a = []() -> decltype([] { return 4; }()) { return 4; };
  auto w = [] { return 9; };
  return a() - 4 + tk2(w);
}
int main() { printf("%d\n", s()); }
