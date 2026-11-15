// [temp.constr.atomic]/3: an atomic constraint not of type bool is a hard
// error. The reference to the function that needed the check is not
// diagnosed again as "invalid reference ... constraints not satisfied"
// (clang 22; clang 19 also stays silent here).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++20
//@ diag: 11:56 error "atomic constraint must be of type 'bool' (found 'const int')"
//@ diag: 12:15 note "while checking constraint satisfaction for function 'f' required here"
struct B { static constexpr int v = 2; };
template <class E> struct F { static int f() requires (E::v) { return 1; } };
int x = F<B>::f();
