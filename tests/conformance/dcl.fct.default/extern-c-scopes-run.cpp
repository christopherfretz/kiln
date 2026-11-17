// [dcl.fct.default]/4, [dcl.link]/7: default arguments belong to a
// declaration's scope; an extern "C" function redeclared in another
// namespace (or at global scope) is the same function, but each scope's
// declarations have their own default arguments, so `A::f()` and `B::f()`
// pass different ones and a later redeclaration in A keeps A's. Found
// running GCC's C++ testsuite. The rejected side:
// extern-c-scopes-neg.cpp.
//@ expect: run
//@ oracle: gcc
namespace A { extern "C" int f(int = 5); }
namespace B { extern "C" int f(int = 6); }
extern "C" int f(int);
int a() { return A::f(); }
int b() { return B::f(); }
extern "C" int f(int x) { return x; }
namespace A { extern "C" int f(int); }
int c() { return A::f(); }
int main() { return a() == 5 && b() == 6 && c() == 5 && f(7) == 7 ? 0 : 1; }
