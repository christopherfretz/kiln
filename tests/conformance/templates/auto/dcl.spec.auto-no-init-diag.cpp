// [dcl.spec.auto.general]/4: a placeholder variable needs an initializer,
// and cannot deduce from void or from a self-reference.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:6 error "declaration of variable 'a' with deduced type 'auto' requires an initializer"
//@ diag: 11:6 error "variable has incomplete type 'void'"
//@ diag: 12:10 error "variable 'c' declared with deduced type 'auto' cannot appear in its own initializer"
//@ diag: 13:12 error "'auto' not allowed in non-static struct member"
auto a;
void f();
auto b = f();
auto c = c + 1;
struct S { auto m = 1; };
