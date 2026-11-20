// [temp.expl.spec]/1: `template<>` before a class-key that declares no
// class (a variable with an elaborated type) is extraneous; a second
// `template` needs its parameter list ([temp.pre]/1). WP-1740.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:1 error "extraneous 'template<>' in declaration of variable 'p'"
//@ diag: 15:1 error "extraneous 'template<>' in declaration of variable 'x'"
//@ diag: 16:21 error "expected '<' after 'template'"
//@ diag: 17:1 error "extraneous 'template<>' in declaration of variable 'q'"
//@ diag: 18:1 error "extraneous 'template<>' in declaration of variable 'final'"
template<class T> struct A { struct B {}; };
struct P {};

template<> class A<int> *p;
template<> struct A<long>::B x;
template<> template void f(int);
template<> struct P q;
template<> struct A<char> final;

// Well-formed: an explicit specialization of a member class.
template<> struct A<short>::B { int i; };
static_assert(sizeof(A<short>::B) == sizeof(int));
