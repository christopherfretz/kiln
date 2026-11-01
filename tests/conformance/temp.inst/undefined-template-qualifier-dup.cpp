// [temp.inst]/2, [basic.lookup.qual.general]/1: naming a member of an undefined template's
// specialization is "implicit instantiation of undefined template"; clang reports it twice
// for a declarator's type, kiln's engine once (spec/diag.md §7.4, `dup(clang)`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:1 error "implicit instantiation of undefined template 'U<int>'" dup(clang)
//@ diag: 12:27 note "template is declared here"
//@ diag: 15:9 error "implicit instantiation of undefined template 'U<int>'"
//@ diag: 12:27 note "template is declared here"
//@ diag: 16:12 error "implicit instantiation of undefined template 'U<int>'" dup(clang)
//@ diag: 12:27 note "template is declared here"
template <class T> struct U;
U<int>::In in;
int q = in.q;
int r = U<int>::v;
struct S { U<int>::In m; };
