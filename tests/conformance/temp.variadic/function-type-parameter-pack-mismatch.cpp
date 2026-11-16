// [dcl.fct]/16, [temp.deduct.call]/1: the pattern of a function parameter
// pack declared with a function type (`T... x(int)`) stays a function type,
// so it is not deduced from a function argument, which decays to a pointer
// (clang and GCC both reject it); the pack must be given explicitly.
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 13:12 error "no matching function for call to 'take'"
//@ diag: 11:28 note "candidate template ignored: could not match 'T (int)' against 'int (*)(int)'"

template <class... T> void take(T... x(int));
int f1(int);
void h() { take(f1); }
