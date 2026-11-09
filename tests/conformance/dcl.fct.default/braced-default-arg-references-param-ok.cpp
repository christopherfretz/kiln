// [dcl.fct.default]/9 valid twin: a parameter in an unevaluated operand of
// a braced default argument, and a global variable named in nested
// lists, are fine (WP-1040).
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
struct S { unsigned long x, y; };
struct P { S s; int z; };
int b = 3;
void f(int a, S s = {sizeof(a), alignof(decltype(a))});
void g(int a, P p = {{1, sizeof a}, b});
void h(S s = {1, 2}, P p = {{}, 0});
