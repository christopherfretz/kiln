// [temp.constr.atomic]/3, [temp.constr.fold]: the notes of an unsatisfied
// atomic constraint print a substituted `sizeof...(P)` as written (clang's
// substituted `SizeOfPackExpr`), and for a `&&` or `||` left in the
// substituted expression (an expanded fold over an enclosing class's pack,
// or over the function's own pack) name the operands that are false, a
// concept-id with its own reason, "and" after the first (clang
// `diagnoseWellFormedUnsatisfiedConstraintExpr`). WP-1405.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 46:3 error "no matching function for call to 'f'"
//@ diag: 38:77 note "candidate template ignored: constraints not satisfied [with Us = <int>]"
//@ diag: 38:35 note "because 'sizeof...(Us) == sizeof...(T)' (1 == 2) evaluated to false"
//@ diag: 47:3 error "no matching function for call to 'g'"
//@ diag: 39:59 note "candidate template ignored: constraints not satisfied [with Us = <int>]"
//@ diag: 39:35 note "because 'char' does not satisfy 'C'"
//@ diag: 36:32 note "because 'sizeof(char) == 4' (1 == 4) evaluated to false"
//@ diag: 39:35 note "and 'short' does not satisfy 'C'"
//@ diag: 36:32 note "because 'sizeof(short) == 4' (2 == 4) evaluated to false"
//@ diag: 48:3 error "no matching function for call to 'h'"
//@ diag: 40:60 note "candidate template ignored: constraints not satisfied [with Us = <int, char>]"
//@ diag: 40:35 note "because 'char' does not satisfy 'C'"
//@ diag: 36:32 note "because 'sizeof(char) == 4' (1 == 4) evaluated to false"
//@ diag: 49:3 error "no matching function for call to 'k'"
//@ diag: 41:59 note "candidate template ignored: constraints not satisfied [with Us = <int>]"
//@ diag: 41:35 note "because 'char' does not satisfy 'C'"
//@ diag: 36:32 note "because 'sizeof(char) == 4' (1 == 4) evaluated to false"
//@ diag: 41:35 note "and 'short' does not satisfy 'C'"
//@ diag: 36:32 note "because 'sizeof(short) == 4' (2 == 4) evaluated to false"
//@ diag: 50:3 error "no matching function for call to 'f2'"
//@ diag: 43:57 note "candidate template ignored: constraints not satisfied [with Us = <int>]"
//@ diag: 43:33 note "because 'sizeof...(Us) == 2' (1 == 2) evaluated to false"
//@ diag: 51:3 error "no matching function for call to 'k2'"
//@ diag: 44:74 note "candidate template ignored: constraints not satisfied [with T = int, Us = <int>]"
//@ diag: 44:42 note "because 'sizeof...(Us) == sizeof(int)' (1 == 4) evaluated to false"
template <class X> concept C = sizeof(X) == 4;
template <class... T> struct S {
  template <class... Us> requires(sizeof...(Us) == sizeof...(T)) static int f(Us...);
  template <class... Us> requires(C<T> && ...) static int g(Us...);
  template <class... Us> requires(C<Us> && ...) static int h(Us...);
  template <class... Us> requires(C<T> || ...) static int k(Us...);
};
template <class... Us> requires(sizeof...(Us) == 2) int f2(Us...);
template <class T, class... Us> requires(sizeof...(Us) == sizeof(T)) int k2(T, Us...);
void use() {
  S<int, int>::f(1);
  S<int, char, short>::g(1);
  S<int>::h(1, 'c');
  S<char, short>::k(1);
  f2(1);
  k2(1, 2);
}
