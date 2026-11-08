// clang `musttail` (WP-809): cleanup scopes, instantiation-time checks,
// lambdas, decl/statement-expression/comma subjects, both if branches.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:57 error "cannot perform a tail call from this return statement"
//@ diag: 34:21 note "jump exits scope of variable with __attribute__((cleanup))"
//@ diag: 35:65 error "non-static member function cannot perform a tail call to non-member function 'f'"
//@ diag: 26 note "'f' declared here"
//@ diag: 35:47 note "tail call required by"
//@ diag: 37:19 error "attribute cannot be applied to a declaration"
//@ diag: 39:37 error "attribute requires that the return value is the result of a function call"
//@ diag: 42:38 error "attribute requires that the return value is the result of a function call"
//@ diag: 42:45 warning "left operand of comma operator has no effect"
//@ diag: 29:52 error "cannot perform a tail call to function 'g' because its signature is incompatible with the calling function"
//@ diag: 31:24 note "in instantiation of function template specialization 'tf<int>' requested here"
//@ diag: 27 note "target function has different return type ('int' expected but has 'long')"
//@ diag: 29:34 note "tail call required by"
//@ diag: 30:54 error "cannot perform a tail call to function 'f' because its signature is incompatible with the calling function"
//@ diag: 32:25 note "in instantiation of function template specialization 'tg<long>' requested here"
//@ diag: 26 note "target function has type mismatch at 1st parameter (expected 'int' but has 'long')"
//@ diag: 30:36 note "tail call required by"
//@ diag: 40:112 error "non-static member function cannot perform a tail call to non-member function 'f'"
//@ diag: 41:46 note "in instantiation of member function 'W<int>::n' requested here"
//@ diag: 26 note "'f' declared here"
//@ diag: 40:94 note "tail call required by"
int f(int);
long g(int);
void cl(int*);
template <class T> T tf(T x) { [[clang::musttail]] return g(x); }
template <class T> int tg(T x) { [[clang::musttail]] return f(x); }
int u1(int x) { return tf(x); }
int u2(long x) { return tg(x); }
int u3(int x) { return tg(x); }
int u4(int x) { int c __attribute__((cleanup(cl))) = 0; [[clang::musttail]] return f(x); }
int u5(int x) { auto l = [](int y) -> int { [[clang::musttail]] return f(y); }; return l(x); }
int u6(int x) { if (x) { [[clang::musttail]] return f(x - 1); } else { __attribute__((musttail)) return f(x + 1); } }
int u7(int x) { [[clang::musttail]] int y = 0; return y; }
int u8(int x) { [[clang::musttail]] [[likely]] return f(x); }
int u9(int x) { [[clang::musttail]] return ({ f(x); }); }
template <class T> struct W { T m(T x) { [[clang::musttail]] return m(x); } int n(int x) { [[clang::musttail]] return f(x); } };
int u10(int x) { W<int> w; return w.m(x) + w.n(x); }
int u11(int x) { [[clang::musttail]] return 0, f(x); }
