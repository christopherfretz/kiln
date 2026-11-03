//@ expect: ast
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ oracle: none
// [dcl.attr.annotation]/3: each annotation is a distinct annotation, even
// with an equal value (P3394R4 Example 2: `g` has five), and each
// instantiation of an annotation is a new one.
[[=1, =1]] [[=1]] int three;
[[=1]] [[=1]] void g();
[[=1, =1, =1]] void g();
template <int N> struct [[=N, =N]] X { };
X<0> x0;
X<1> x1;
X<1> x1_again;
