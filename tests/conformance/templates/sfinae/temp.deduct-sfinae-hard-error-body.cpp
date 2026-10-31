// [temp.deduct]/8: an error in the instantiated body is not in the immediate context.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:57 error "static assertion failed due to requirement 'sizeof(int) == 0'"
//@ diag: 9:9 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 7:67 note "expression evaluates to '4 == 0'"
template <class T> constexpr int f(T t) { static_assert(sizeof(T) == 0); return t; }
constexpr int f(...) { return 2; }
int r = f(1);
