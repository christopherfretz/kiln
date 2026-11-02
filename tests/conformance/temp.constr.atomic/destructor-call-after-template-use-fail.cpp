// [temp.constr.atomic]/3, [class.access]/4: the destructor-call requirement
// is checked after substitution like any other expression; an
// inaccessible destructor makes it invalid.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "static assertion failed"
//@ diag: 12:15 note "because 'P' does not satisfy 'D'"
//@ diag: 9:51 note "because 'v.~T()' would be invalid: '~P' is a private member of 'P'"
template <class T> concept D = requires(T v) { v.~T(); };
class P { ~P(); };
template <class X> void h() { P *p = nullptr; (void)p; }
static_assert(D<P>);
