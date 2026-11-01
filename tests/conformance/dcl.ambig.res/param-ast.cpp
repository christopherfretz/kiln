// [dcl.ambig.res]/1: in a parameter-declaration-clause `T(a)` is a
// parameter named `a` with redundant parentheses; `T(U)` with `U` a type
// is an unnamed parameter of function type, adjusted to a pointer.
//@ expect: ast
struct T { T(); T(int); };
typedef int U;
void g1(T(a));
void s13() { void g2(T(a)); }
void g3(T(U));
void g4(T(*)(U));
void g5(T((b)));
