// [dcl.type.class.deduct]/2, [dcl.spec.auto]: a variable declaration that
// is not a definition cannot deduce its class type.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:10 error "declaration of variable 'e' with deduced type 'B' requires an initializer"
//@ diag: 9:21 error "declaration of variable 'sm' with deduced type 'B' requires an initializer"
template<class T> struct B { B(T); };
extern B e;
struct S { static B sm; };
