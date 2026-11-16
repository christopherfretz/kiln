// [temp.friend]/1, [dcl.meaning.general]/2: a friend with a dependent
// qualifier whose signature matches no member of the class the
// instantiation names is an error at that instantiation, with the
// candidates noted (WP-1495).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 11:43 error "friend declaration of 'f' does not match any declaration in 'P'"
//@ diag: 13:6 note "in instantiation of template class 'S<P>' requested here"
//@ diag: 12:16 note "candidate function has type mismatch at 1st parameter (expected 'long' but has 'int')"
template<class T> class S { friend int T::f(long); };
struct P { int f(int); };
S<P> s;
