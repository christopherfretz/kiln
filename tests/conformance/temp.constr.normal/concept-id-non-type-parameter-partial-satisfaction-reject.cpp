// [temp.constr.atomic]/3, [temp.constr.op]: the rejected side of
// concept-id-non-type-parameter-partial-satisfaction.cpp. In a conjunction
// the atom naming the non-type parameter whose argument failed is
// unsatisfied; in a disjunction whose other operand is false that operand
// is reported. A failed conversion of a non-type argument has no note of
// its own (as clang 22). Found
// running GCC's C++ testsuite (WP-1598).
//@ std: c++20
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 22:9 error "no matching function for call to 'f'"
//@ diag: 21:49 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 21:35 note "because substituted constraint expression is ill-formed: type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 25:9 error "no matching function for call to 'h'"
//@ diag: 24:51 note "candidate template ignored: constraints not satisfied [with T = double]"
//@ diag: 23:39 note "because 'double' does not satisfy 'IsInt'"
//@ diag: 19:36 note "because '__is_same(double, int)' evaluated to false"
template <class T> concept IsInt = __is_same(T, int);
template <class T, int N> concept CC = IsInt<T> && (N > 0);
template <class T> requires CC<T, T::value> int f(T) { return 1; }
int a = f(1);
template <class T, int N> concept C = IsInt<T> || (N > 0);
template <class T> requires C<T, (T*)nullptr> int h(T) { return 3; }
int c = h(1.0);
