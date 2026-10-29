// [dcl.pre]/12, [temp.res.general]/6: a dependent static_assert is
// evaluated in each instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:45 error "static assertion failed due to requirement '0 > 0': N must be positive"
//@ diag: 12:8 note "in instantiation of template class 'Pos<0>' requested here"
//@ diag: 13:47 error "static assertion failed due to requirement 'sizeof(char) > 4': too small"
//@ diag: 15:11 note "in instantiation of template class 'Big<char>' requested here"
//@ diag: 13:57 note "expression evaluates to '1 > 4'"
template <int N> struct Pos { static_assert(N > 0, "N must be positive"); };
Pos<1> ok;
Pos<0> bad;
template <class T> struct Big { static_assert(sizeof(T) > 4, "too small"); };
Big<double> fine;
Big<char> small;
