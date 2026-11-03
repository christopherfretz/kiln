// [temp.arg.general]/1, [temp.names]/1: a variable template or concept named
// without template arguments is not a type, a class template or a value.
// clang 19 takes it as an expression (a concept: a concept-id missing its
// arguments); an invalid argument ends the template-argument-list.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 48:3 error "use of variable template 'v' requires template arguments"
//@ diag: 36:22 note "template is declared here"
//@ diag: 49:3 error "too few template arguments for concept 'C'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 50:3 error "use of variable template 'v' requires template arguments"
//@ diag: 36:22 note "template is declared here"
//@ diag: 51:3 error "too few template arguments for concept 'C'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 52:3 error "use of variable template 'v' requires template arguments"
//@ diag: 36:22 note "template is declared here"
//@ diag: 53:3 error "too few template arguments for concept 'C'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 54:11 error "use of variable template 'N::w' requires template arguments"
//@ diag: 39:22 note "template is declared here"
//@ diag: 55:6 error "too few template arguments for concept 'D'"
//@ diag: 40:26 note "template is declared here"
//@ diag: 61:14 error "use of variable template 'v' requires template arguments"
//@ diag: 36:22 note "template is declared here"
//@ diag: 61:22 error "use of variable template 'v' requires template arguments"
//@ diag: 36:22 note "template is declared here"
//@ diag: 61:30 error "use of variable template 'v' requires template arguments"
//@ diag: 36:22 note "template is declared here"
//@ diag: 61:38 error "too few template arguments for concept 'C'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 64:9 error "use of variable template 'v' requires template arguments"
//@ diag: 36:22 note "template is declared here"
//@ diag: 65:10 error "use of concept 'C' requires template arguments"
//@ diag: 37:26 note "template is declared here"
template <class> int v = 0;
template <class> concept C = true;
namespace N {
template <class> int w = 0;
template <class> concept D = true;
}
template <class T> struct A {};
template <template <class> class TT> struct B {};
template <int N> struct I {};
template <class... Ts> struct P {};

// Where a type, a class template or a value is expected.
A<v> a1;
A<C> a2;
B<v> b1;
B<C> b2;
I<v> i1;
I<C> i2;
P<int, N::w, char> p1;
A<N::D> a3;

// In explicit template arguments of a call.
template <int> void f();
template <class> void g();
template <template <class> class> void h();
void t() { f<v>(); g<v>(); h<v>(); g<C>(); }

// As an expression.
int x = v;
bool b = C;
