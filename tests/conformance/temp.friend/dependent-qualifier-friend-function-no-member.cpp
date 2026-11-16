// [temp.friend]/1, [dcl.meaning.general]/2: a friend with a dependent
// qualifier that names no member of the instantiation's class is
// ill-formed at that instantiation (as GCC 16.1 diagnoses it) (WP-1495).
//@ expect: compile-fail
//@ std: c++17
//@ skip(clang): clang 22 drops a dependent-qualified friend that names no member, without a diagnostic
//@ strict-diags
//@ diag: 10:43 error "friend declaration of 'h' does not match any declaration in 'P'"
//@ diag: 12:6 note "in instantiation of template class 'S<P>' requested here"
template<class T> class S { friend int T::h(long); };
struct P { int f(); };
S<P> s;
