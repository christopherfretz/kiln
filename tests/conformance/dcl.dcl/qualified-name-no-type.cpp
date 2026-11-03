// [dcl.meaning.general]/2, [dcl.spec.general]/3, [dcl.type.general]/2:
// namespace-scope recovery for a declaration that starts with a
// qualified name that names no type (clang's `ParseImplicitInt` and
// `ParseDeclGroup`; WP-411).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 59:4 error "no type named 'f' in namespace 'R'"
//@ diag: 60:4 error "no type named 'h' in namespace 'R'"
//@ diag: 61:6 error "no type named 'f' in namespace 'R'"
//@ diag: 62:4 error "no type named 'f' in namespace 'R'"
//@ diag: 63:4 error "no type named 'f' in namespace 'R'"
//@ diag: 64:4 error "no type named 'f' in namespace 'R'"
//@ diag: 65:4 error "no type named 'v' in namespace 'R'"
//@ diag: 66:4 error "no type named 'f' in namespace 'R'"
//@ diag: 67:4 error "no type named 'f' in namespace 'R'"
//@ diag: 68:4 error "no type named 'x' in 'S'"
//@ diag: 72:4 error "a type specifier is required for all declarations"
//@ diag: 73:4 error "a type specifier is required for all declarations"
//@ diag: 74:4 error "a type specifier is required for all declarations"
//@ diag: 75:4 error "a type specifier is required for all declarations"
//@ diag: 76:4 error "a type specifier is required for all declarations"
//@ diag: 77:4 error "a type specifier is required for all declarations"
//@ diag: 78:4 error "a type specifier is required for all declarations"
//@ diag: 79:1 error "a type specifier is required for all declarations"
//@ diag: 80:1 error "a type specifier is required for all declarations"
//@ diag: 85:4 error "no type named 'f' in namespace 'R'"
//@ diag: 85:13 error "expected ';' after top level declarator"
//@ diag: 86:4 error "no type named 'f' in namespace 'R'"
//@ diag: 86:14 error "expected ';' after top level declarator"
//@ diag: 87:4 error "no type named 'f' in namespace 'R'"
//@ diag: 87:15 error "expected ';' after top level declarator"
//@ diag: 88:4 error "no type named 'f' in namespace 'R'"
//@ diag: 88:14 error "expected ';' after top level declarator"
//@ diag: 89:4 error "no type named 'f' in namespace 'R'"
//@ diag: 89:17 error "expected ';' after top level declarator"
//@ diag: 90:4 error "no type named 'f' in namespace 'R'"
//@ diag: 90:17 error "expected ';' after top level declarator"
//@ diag: 91:4 error "no type named 'f' in namespace 'R'"
//@ diag: 91:17 error "expected ';' after top level declarator"
//@ diag: 92:4 error "no type named 'f' in namespace 'R'"
//@ diag: 92:12 error "expected ';' after top level declarator"
//@ diag: 93:4 error "no type named 'f' in namespace 'R'"
//@ diag: 94:4 error "no type named 'f' in namespace 'R'"
//@ diag: 94:8 error "expected ';' after top level declarator"
//@ diag: 97:4 error "no type named 'g' in namespace 'R'"
//@ diag: 97:8 error "expected ';' after top level declarator"
//@ diag: 101:8 error "no member named 'q' in namespace 'R'"
//@ diag: 102:7 warning "extra qualification on member 'q2'"
//@ diag: 102:7 error "no member named 'q2' in the global namespace"

namespace R { void f(int); int v; int w[2]; }
struct S { static int x; void m(int); };
int v;
typedef int X;

// A `(` that can start a declarator: the name was meant as a type.
R::f(v);
R::h(v);
::R::f(v);
R::f(X);
R::f(*p)[2];
R::f(R::v);
R::v(v);
R::f *p1;
R::f const c1 = 1;
S::x(v);

// Otherwise the name is the declarator-id of a declaration without a
// type, which redeclares nothing.
R::f(1);
R::f;
R::f(int);
R::h(int);
R::h;
R::h = 1;
S::m;
f(int);
f(int) {}

// A later declarator of a group whose type is invalid takes no
// initializer; the missing `;` is reported after the `(` the function
// declarator check looked at.
R::f(v), g1(v);
R::f(v), g2 (1);
R::f(v), (g3)(1);
R::f(v), *g4(v);
R::f(v), g5, g6(1);
R::f(v) = v, g7(1);
R::f(v), g8(int) = 2;
R::f(v), g9 = v;
R::f(v), g10, g11(int), g12[2];
R::f(v) int y;

// A missing `;` with a fix-it, after the previous token.
R::g(v)
int after;

// No member of that name in the namespace ([dcl.meaning.general]/2.2).
int R::q;
int ::q2 = 1;
