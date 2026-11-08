// clang `musttail` sema checks (WP-809; clang SemaStmt checkMustTailAttr):
// signature, variadic, noreturn, member/non-member, call shape, trivial
// temporaries, jump scope, statement kind and arguments. The callee notes
// omit the column: kiln points at the name, clang at the decl begin.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 52:37 error "cannot perform a tail call to function 'g' because its signature is incompatible with the calling function"
//@ diag: 42 note "target function has different return type ('int' expected but has 'long')"
//@ diag: 52:19 note "tail call required by"
//@ diag: 53:37 error "cannot perform a tail call to function 'h' because its signature is incompatible with the calling function"
//@ diag: 43 note "target function has different number of parameters (expected 1 but has 2)"
//@ diag: 53:19 note "tail call required by"
//@ diag: 54:37 error "attribute may not be used with variadic functions"
//@ diag: 55:37 error "attribute may not be used with no-return-attribute functions"
//@ diag: 56:42 error "non-member function cannot perform a tail call to non-static member function 'm'"
//@ diag: 46 note "'m' declared here"
//@ diag: 56:24 note "tail call required by"
//@ diag: 58:37 error "attribute requires that the return value is the result of a function call"
//@ diag: 59:38 error "attribute requires that the return value is the result of a function call"
//@ diag: 60:39 error "non-static member function cannot perform a tail call to non-member function 'f'"
//@ diag: 41 note "'f' declared here"
//@ diag: 60:21 note "tail call required by"
//@ diag: 61:44 error "cannot perform a tail call to function 'm' because its signature is incompatible with the calling function"
//@ diag: 60 note "target function is a member of different class (expected 'T' but has 'S')"
//@ diag: 61:26 note "tail call required by"
//@ diag: 62:37 error "tail call requires that the return value, all parameters, and any temporaries created by the expression are trivially destructible"
//@ diag: 63:23 error "cannot perform a tail call from this return statement"
//@ diag: 63:20 note "jump exits scope of variable with non-trivial destructor"
//@ diag: 64:20 error "attribute only applies to return statements"
//@ diag: 66:43 error "'cold' attribute cannot be applied to a statement"
//@ diag: 67:30 error "attribute requires that the return value is the result of a function call"
//@ diag: 69:38 error "cannot perform a tail call to function 'fp' because its signature is incompatible with the calling function"
//@ diag: 68 note "target function has type mismatch at 1st parameter (expected 'long' but has 'int')"
//@ diag: 69:20 note "tail call required by"
//@ diag: 70:43 error "non-member function cannot perform a tail call to non-static member function 'operator()'"
//@ diag: 46 note "'operator()' declared here"
//@ diag: 70:25 note "tail call required by"
//@ diag: 71:24 error "cannot perform a tail call from this return statement"
//@ diag: 71:18 note "jump exits try block"
//@ diag: 73:20 error "attribute takes no arguments"
int f(int);
long g(int);
int h(int, int);
int v(int, ...);
[[noreturn]] int nr(int);
struct S { int m(int); static int sm(int); S(); ~S(); int operator()(int); };
struct T { int m(int); };
struct D { ~D(); };
int dd(D);
D mk();
int fd(const D&);
int t1(int x) { [[clang::musttail]] return g(x); }
int t2(int x) { [[clang::musttail]] return h(x, x); }
int t3(int x) { [[clang::musttail]] return v(x); }
int t4(int x) { [[clang::musttail]] return nr(x); }
int t5(int x) { S s; [[clang::musttail]] return s.m(x); }
int t6(int x) { [[clang::musttail]] return S::sm(x); }
int t7(int x) { [[clang::musttail]] return x; }
void t8(int x) { [[clang::musttail]] return; }
int S::m(int x) { [[clang::musttail]] return f(x); }
int T::m(int x) { S s; [[clang::musttail]] return s.m(x); }
int t9(int x) { [[clang::musttail]] return fd(D()); }
int t10(int x) { D d; [[clang::musttail]] return f(x); }
int t11(int x) { [[clang::musttail]] x = 1; return x; }
int t12(int x) { __attribute__((musttail)) return f(x); }
int t13(int x) { __attribute__((musttail, cold)) return f(x); }
S::S() { [[clang::musttail]] return; }
int (*fp)(long);
int t14(int x) { [[clang::musttail]] return fp(x); }
int t15(int x) { S s; [[clang::musttail]] return s(x); }
int t16(int x) { try { [[clang::musttail]] return f(x); } catch (...) {} return 0; }
int t17(int x) { [[clang::musttail]] return (f(x)); }
int t18(int x) { [[clang::musttail(1)]] return f(x); }
