//@ expect: ast
//@ oracle: clang
// [expr.prim.req]: a non-dependent requires-expression is a bool prvalue,
// evaluated at once ([expr.prim.req.general]/5).
struct S { int x; };
template <class T> concept Big = sizeof(T) > 4;
bool b = requires(S s) { s.x; };
bool c = requires(S s) { typename S; { s.x } noexcept; requires sizeof(int) == 4; };
bool d = requires(S s) { { s.x } -> Big; };
bool e = requires { requires false; };
using T2 = int;
template <class T> struct X {};
template <class T> using A = X<T>;
bool f = requires { typename T2; typename X<int>; typename A<int>; };
