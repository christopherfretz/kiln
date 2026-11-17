// [temp.constr.atomic]/3: the rejected side of
// concept-id-default-argument-substitution-failure.cpp. `sizeof(U) > 0`
// names the defaulted parameter, whose argument `typename T::type` is
// invalid for `int`: the atom is not satisfied and the candidate is
// dropped, with the failed substitution as the reason. Found running
// GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ diag: 12:9 error "no matching function for call to 'h'"
template <typename T, typename U = typename T::type> concept D = sizeof(U) > 0;
template <D T> int h(T);
int x = h(0);
