// [dcl.meaning.general]/2, [basic.link]/11: 'R::v;' at namespace scope is a declaration
// with no type-specifier (an error) that clang does not link into R::v's
// redeclaration chain, so a later 'int R::v;' redeclares the valid 'extern int v'
// and the previous-declaration note points there; an invalid redeclaration
// (redefinition, type mismatch) is not linked either (WP-421).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:4 error "a type specifier is required for all declarations"
//@ diag: 19:4 error "a type specifier is required for all declarations"
//@ diag: 21:4 error "a type specifier is required for all declarations"
//@ diag: 22:4 error "a type specifier is required for all declarations"
//@ diag: 23:9 error "redefinition of 't' with a different type: 'long' vs 'int'"
//@ diag: 16:61 note "previous definition is here"
//@ diag: 24:8 error "redefinition of 't'"
//@ diag: 16:61 note "previous definition is here"
namespace R { extern int v; extern int w; extern int u; int t; }
R::v;
int R::v;
R::w = 1;
int R::w = 2;
R::u;
R::u;
long R::t;
int R::t;
int main() { return R::v + R::w; }
