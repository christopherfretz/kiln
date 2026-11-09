// [dcl.type.auto.deduct]/4, [temp.deduct.call]/4, [dcl.spec.auto.general]/5,
// [temp.param]/6.3: a placeholder under a pointer-to-member declarator
// deduces only from a pointer to member of the same class (no
// derived-to-base adjustment, no null pointer constant) with matching
// parameters; `new auto Q::*` and `template <auto Q::* P>` report the same
// failure in their own words. `decltype(auto)` stands alone: it cannot be
// the pointee of a member pointer, and is the return type only of the
// function declarator next to the declarator-id.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:11 error "variable 'p1' with type 'auto Q::*' has incompatible initializer of type 'int R::*'"
//@ diag: 32:11 error "variable 'p2' with type 'auto Q::*' has incompatible initializer of type 'int'"
//@ diag: 33:11 error "variable 'p3' with type 'auto Q::*' has incompatible initializer of type 'std::nullptr_t'"
//@ diag: 34:11 error "variable 'p4' with type 'auto Q::*' has incompatible initializer of type 'int *'"
//@ diag: 35:12 error "variable 'p5' with type 'auto (Q::*)(long)' has incompatible initializer of type 'double (Q::*)(int)'"
//@ diag: 36:12 error "variable 'p6' with type 'auto (R::*)(int)' has incompatible initializer of type 'double (Q::*)(int)'"
//@ diag: 37:12 error "variable 'p7' with type 'auto (Q::*)(int) const' has incompatible initializer of type 'double (Q::*)(int)'"
//@ diag: 38:16 error "variable 'p8' with type 'auto Q::*Q::*' has incompatible initializer of type 'int Q::*'"
//@ diag: 39:16 error "cannot form pointer to 'decltype(auto)'"
//@ diag: 40:25 error "'decltype(auto)' can only be used as a return type in a function declaration"
//@ diag: 41:23 error "'decltype(auto)' can only be used as a return type in a function declaration"
//@ diag: 42:12 error "new expression for type 'auto Q::*' has incompatible constructor argument of type 'int R::*'"
//@ diag: 43:12 error "new expression for type 'auto *' has incompatible constructor argument of type 'int'"
//@ diag: 44:12 error "new expression for type 'auto Q::*' requires a constructor argument"
//@ diag: 46:3 error "non-type template parameter 'P' with type 'auto Q::*' has incompatible initializer of type 'int R::*'"
//@ diag: 45:21 note "template parameter is declared here"
struct Q { int m; double f(int); };
struct R { int m; };
double g(int);
int x;
auto Q::* p1 = &R::m;
auto Q::* p2 = 0;
auto Q::* p3 = nullptr;
auto Q::* p4 = &x;
auto (Q::* p5)(long) = &Q::f;
auto (R::* p6)(int) = &Q::f;
auto (Q::* p7)(int) const = &Q::f;
auto Q::* Q::* p8 = &Q::m;
decltype(auto) Q::* d1 = &Q::m;
decltype(auto) (Q::* d2)(int) = &Q::f;
decltype(auto) (*d3())(int);
auto *n1 = new auto Q::*(&R::m);
auto *n2 = new auto *(0);
auto *n3 = new auto Q::*;
template <auto Q::* P> struct T {};
T<&R::m> t1;
