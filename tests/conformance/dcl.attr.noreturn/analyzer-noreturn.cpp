// clang's GNU-spelled `analyzer_noreturn` (Attr.td AnalyzerNoReturn,
// `__attribute__` spelling only): it applies to functions and to
// function-pointer variables, takes no arguments, and has no effect but
// in the CFG, where a call to such a function ends the path, so
// -Wreturn-type does not fire after it (unlike [[noreturn]] it is not
// checked in the callee and does not change codegen). BDE's
// bsls_annotation.h defines BSLS_ANNOTATION_ANALYZER_NORETURN as it under
// clang, and the reference workload builds with -Werror.
//@ expect: compile-fail
//@ args: -Wall
//@ strict-diags
//@ diag: 20:23 warning "'analyzer_noreturn' attribute only applies to functions, methods and blocks"
//@ diag: 21:26 error "'analyzer_noreturn' attribute takes no arguments"
//@ diag: 22:3 warning "unknown attribute 'clang::analyzer_noreturn' ignored"
//@ diag: 23:3 warning "unknown attribute 'gnu::analyzer_noreturn' ignored"

void fail() __attribute__((analyzer_noreturn));
void fail2() __attribute__((__analyzer_noreturn__));
void (*handler)() __attribute__((analyzer_noreturn));   // OK
int v1 __attribute__((analyzer_noreturn));
void f3() __attribute__((analyzer_noreturn(1)));
[[clang::analyzer_noreturn]] void f4();
[[gnu::analyzer_noreturn]] void f5();
struct S {
  void m() __attribute__((analyzer_noreturn));
  static void sm() __attribute__((analyzer_noreturn));
};

int g1(int x) { if (x) return 1; fail(); }              // no -Wreturn-type
int g2() { fail2(); }
int g3(S &s) { s.m(); }
int g4() { S::sm(); }
int g5() { handler(); }                                 // the variable counts
void h() __attribute__((analyzer_noreturn));
void h() { }                                            // no -Winvalid-noreturn
