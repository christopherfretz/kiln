// [temp.param]/2, [dcl.type.general]/3: a class defined in a lambda body is
// not defined in the enclosing type-id, so a lambda in a non-type template
// parameter's type, a type-parameter default or a parameter's decltype may
// define local classes (clang accepts all of these).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
template<decltype([] { struct P { int v; }; return 12; }()) N> int nttp() { return N; }
template<class T = decltype([] { struct Q { int w; }; return Q{}; }())> struct D { T t; };
void f(decltype([] { enum E { e }; return e; }()) x);
auto g(int x = sizeof(decltype([] { union U { int u; }; return U{}; }()))) -> int;
D<> d;
int n = sizeof(d.t.w);
