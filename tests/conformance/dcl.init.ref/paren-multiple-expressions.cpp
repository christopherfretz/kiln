// [dcl.init.ref]/1, [dcl.init.general]/16.2: a reference is bound to one
// expression; a parenthesized expression-list of several is "reference
// cannot be initialized with multiple values" at the declarator, also
// for a reference to an aggregate (P0960's parenthesized aggregate
// initialization initializes objects, [dcl.init.general]/16.6.2.2; clang
// `FK_TooManyInitsForReference`). GCC accepts the aggregate case.
//@ expect: compile-fail
//@ strict-diags
//@ xfail(gcc): GCC binds a reference to an aggregate initialized from a parenthesized list
//@ diag: 17:10 error "reference cannot be initialized with multiple values"
//@ diag: 18:6 error "reference cannot be initialized with multiple values"
//@ diag: 19:7 error "reference cannot be initialized with multiple values"
//@ diag: 20:42 error "reference cannot be initialized with multiple values"
//@ diag: 21:12 note "in instantiation of function template specialization 'f<int>' requested here"
//@ diag: 20:55 error "reference cannot be initialized with multiple values"
struct A { int i, j; };
const A& ra(1, 2);
int& r2(1, 2);
int&& r3(1, 2, 3);
template<class T> void f(T t) { const A& r(t, 2); T&& x(t, t); }
void g() { f(1); }
