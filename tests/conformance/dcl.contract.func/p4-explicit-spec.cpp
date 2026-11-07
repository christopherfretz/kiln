// [dcl.contract.func]/4, [temp.expl.spec]/12 (P2900R14 §3.4.7): an
// explicit specialization is a function of its own whose first declaration
// is its first written one: the template's seq does not count, so a seq
// that differs from the template's (or none) is fine, but a later
// declaration of the same explicit specialization adds no seq and repeats
// it exactly. GCC 16.1 diagnoses neither redeclaration of an explicit
// specialization, so there is no oracle (spec/contracts.md §9).
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ diag: 21:25 error "declaration adds function contract specifiers to 'f'"
//@ diag: 20:18 note "previous declaration is here"
//@ diag: 25:27 error "function contract specifiers of 'f' do not match the previous declaration"
//@ diag: 24:18 note "previous declaration is here"
//@ diag: 30:27 error "declaration adds function contract specifiers to 'g'"
//@ diag: 29:18 note "previous declaration is here"
//@ diag: 33:33 error "declaration adds function contract specifiers to 'm'"
//@ diag: 32:26 note "previous declaration is here"
template <class T> void f(T) {}
template <> void f(int);
template <> void f(int) pre(true) {}
template <> void f(long) pre(true);
template <> void f(long) pre(true) {}
template <> void f(short) pre(true);
template <> void f(short) pre(false) {}
template <class T> void g(T) pre(true) {}
template <> void g(int) {}
template <> void g(long) pre(false) {}
template <> void g(short);
template <> void g(short) pre(true) {}
template <class T> struct C { void m(T) pre(true) {} };
template <> void C<int>::m(int);
template <> void C<int>::m(int) pre(true) {}
template <> void C<long>::m(long) pre(false) {}
template <> void C<short>::m(short) {}
