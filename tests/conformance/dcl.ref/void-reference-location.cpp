// [dcl.ref]/1: "cannot form a reference to 'void'" points at the `&` or
// `&&` that forms it, as clang's ReferenceTypeLoc does: in a plain
// declarator, and in a template's field, static data member, member
// typedef, member function parameter or local variable whose dependent
// reference type becomes `void&` on substitution (Sema::ref_locs). An alias
// template's is reported at its declaration. [temp.deduct.call]/1: the call's
// arity is checked against the written parameters before the explicit
// template arguments are substituted, so `f<void>()` of `f(T &)` is not
// viable for its argument count, not a substitution failure.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 40:6 error "cannot form a reference to 'void'"
//@ diag: 41:3 error "cannot form a reference to 'void'"
//@ diag: 42:33 error "cannot form a reference to 'void'"
//@ diag: 43:9 note "in instantiation of template class 'X<void>' requested here"
//@ diag: 45:12 error "no matching function for call to 'f'"
//@ diag: 44:25 note "candidate function template not viable: requires 1 argument, but 0 were provided"
//@ diag: 46:16 error "cannot form a reference to 'void'"
//@ diag: 47:14 error "cannot form a reference to 'void'"
//@ diag: 48:13 error "cannot form a reference to 'void'"
//@ diag: 49:17 error "cannot form a reference to 'void'"
//@ diag: 50:20 error "cannot form a reference to 'void'"
//@ diag: 51:1 note "in instantiation of template type alias 'Ref' requested here"
//@ diag: 52:12 error "cannot form a reference to 'void'"
//@ diag: 53:40 error "cannot form a reference to 'void'"
//@ diag: 54:9 note "in instantiation of template class 'B<void>' requested here"
//@ diag: 55:40 error "cannot form a reference to 'void'"
//@ diag: 56:9 note "in instantiation of template class 'C<void>' requested here"
//@ diag: 57:33 error "cannot form a reference to 'void'"
//@ diag: 58:15 note "in instantiation of function template specialization 'l<void>' requested here"
//@ diag: 59:33 error "cannot form a reference to 'void'"
//@ diag: 60:9 note "in instantiation of template class 'E<void>' requested here"
//@ diag: 61:41 error "cannot form a reference to 'void'"
//@ diag: 62:9 note "in instantiation of template class 'F<void>' requested here"
//@ diag: 64:13 error "no matching function for call to 'k'"
//@ diag: 63:25 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 64:25 error "no matching function for call to 'k'"
//@ diag: 63:25 note "candidate function template not viable: requires 2 arguments, but 3 were provided"
typedef void V;
void &r1 = *(int*)0;
V &r2;
template <class T> struct X { T &m; };
X<void> x;
template <class T> void f(T &);
void g() { f<void>(); }
using R = void &;
using R2 = V &&;
void h(void &p);
struct Y { void &q; };
template <class T> using Ref = T&;
Ref<void> rv;
const void &cr = 1;
template <class T> struct B { static T &s; };
B<void> b;
template <class T> struct C { void f(T &p); };
C<void> c;
template <class T> void l() { T &v = *(int*)0; }
template void l<void>();
template <class T> struct E { T &&rr; };
E<void> e;
template <class T> struct F { typedef T &type; };
F<void> fv;
template <class T> void k(T &, int);
void g2() { k<void>(1); k<void>(1, 2, 3); }
