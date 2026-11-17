// [temp.constr.atomic]/3, [temp.constr.op]: the rejected side of
// concept-id-pack-parameter-partial-satisfaction.cpp. The fold over the
// pack names the failed argument; the other operand of the disjunction is
// evaluated and false, so both are reported. Found running GCC's C++
// testsuite (WP-1579).
//@ std: c++20
//@ expect: compile-fail
//@ requires: clang>=22
//@ strict-diags
//@ diag: 16:9 error "no matching function for call to 'pc'"
//@ diag: 15:57 note "candidate template ignored: constraints not satisfied [with T = int]"
//@ diag: 15:44 note "because substituted constraint expression is ill-formed: type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 14:45 note "and 'sizeof(int) > 100' (4 > 100) evaluated to false"
template <class T, class... A> concept PC = sizeof(T) > 100 || (sizeof(A) + ... + 0) > 0;
template <class T> requires PC<T, typename T::nope> int pc(T);
int y = pc(0);
