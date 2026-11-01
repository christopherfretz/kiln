// [namespace.udecl]/10: an inheriting using-declaration cannot be
// repeated in a class (also after instantiation); the redeclaration is
// dropped, so the constructors stay usable without further errors.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:37 error "redeclaration of using declaration"
//@ diag: 13:25 note "previous using declaration"
//@ diag: 15:56 error "redeclaration of using declaration"
//@ diag: 16:6 note "in instantiation of template class 'F<B>' requested here"
//@ diag: 15:44 note "previous using declaration"
struct B { B(int); B(double); };
struct C { C(int); };
struct D : B { using B::B; using B::B; };
D d(1);
template <class T> struct F : T { using T::T; using T::T; };
F<B> f(2.0);
struct G : B, C { using B::B; using C::C; G(int); };
G g(1);
