// [dcl.fct.default]/9: a parameter shall not appear as a potentially-
// evaluated expression in a default argument, also inside a braced one
// and its nested lists; every such use is reported (WP-1040).
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:22 error "default argument references parameter 'a'"
//@ diag: 14:33 error "default argument references parameter 'b'"
//@ diag: 14:37 error "default argument references parameter 'a'"
//@ diag: 15:35 error "default argument references parameter 'a'"
struct S { int x, y; };
struct P { S s; int z; };
void f(int a, S s = {a, 1});
void g(int a, int b, P p = {{1, b}, a});
void h(int a, S s = {.x = 1, .y = a});
