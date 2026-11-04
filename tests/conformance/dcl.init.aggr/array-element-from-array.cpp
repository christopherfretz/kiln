// [dcl.init.aggr]/4.2, /16, [dcl.init.general]/16.6: an expression of
// array type in a braced list for an array member does not initialize the
// member (no implicit conversion between array types); brace elision
// initializes the member's first element from it, which fails. The error
// is located at the written element, also in a template pattern and in a
// nested aggregate (WP-613: the implicit value-initialization of an
// omitted array member is never such an element).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:10 error "cannot initialize an array element of type 'char' with an lvalue of type 'char[4]'"
//@ diag: 22:11 error "cannot initialize an array element of type 'char' with an lvalue of type 'char[4]'"
//@ diag: 23:13 error "cannot initialize an array element of type 'char' with an lvalue of type 'char[4]'"
//@ diag: 24:16 error "cannot initialize an array element of type 'char' with an lvalue of type 'char[4]'"
//@ diag: 27:38 error "cannot initialize an array element of type 'char' with an lvalue of type 'char[4]'"
//@ diag: 28:38 error "cannot initialize an array element of type 'char' with an lvalue of type 'char[4]'"
struct S { unsigned short fam; char path[4]; };
struct O { S s; int n; };
char g[4];

void f() {
  S a{1, g};
  O o{{1, g}};
  S b[2]{1, g};
  char c[2][4]{g};
}

template <class T> void t() { S a{1, g}; }
template <class T> void u() { O o{1, g}; }
