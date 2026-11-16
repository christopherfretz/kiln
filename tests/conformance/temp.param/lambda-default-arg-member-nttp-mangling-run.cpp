// [temp.param]/10, [expr.prim.lambda.closure]/2: in a member template of
// class template `S<int>`, a lambda in the default of a type parameter
// (`decltype([] {})`, an unevaluated operand) is the closure substituted
// with `S<int>`, shared by every use, while a lambda as the default of a
// non-type parameter is a new closure at each use, numbered in `S<int>`:
// `m` uses `UlvE_`, `j` `UlvE1_` (the second number goes to `k`'s
// default in `S<int>`), `k<1>` and `k<2>` `UlvE2_` and `UlvE3_`, as
// clang 19 and 22 number them
// (WP-1497). Inputs/lambda-default-arg-member-nttp-mangling-defs.cpp
// defines each under clang's name with an asm label, so the program links
// only if kiln's references carry those names.
//@ std: c++20
//@ expect: run
//@ args: %{test_dir}/Inputs/lambda-default-arg-member-nttp-mangling-defs.cpp
//@ stdout: 1 1 2 3 4 4
extern "C" int printf(const char *, ...);
template <class T> struct S {
  template <class U = decltype([] {})> static int m();
  template <int N, auto F = [] {}> static int k();
  template <int N, class U = decltype([] {})> static int j();
};
int main() {
  printf("%d %d %d %d %d %d\n", S<int>::m(), S<int>::m(), S<int>::k<1>(), S<int>::k<2>(),
         S<int>::j<1>(), S<int>::j<2>());
}
