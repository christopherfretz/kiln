// [dcl.inline]/1: inline on a namespace-scope variable, a static data member
// and a function is accepted; before C++17 an inline variable is an
// extension (-Wc++17-extensions).
//@ expect: compile-ok
//@ strict-diags
//@ args: -std=c++14
//@ diag: 10:1 warning "inline variables are a C++17 extension [-Wc++17-extensions]"
//@ diag: 11:19 warning "inline variables are a C++17 extension [-Wc++17-extensions]"
//@ diag: 13:1 warning "inline variables are a C++17 extension [-Wc++17-extensions]"
inline int v = 1;
struct S { static inline int j = 1; inline int f(); static inline int g() { return 0; } };
inline int S::f() { return j; }
inline struct T { int a; } t;
int use() { return v + t.a + S::g(); }
