// clang's GNU-spelled `analyzer_noreturn` (Attr.td AnalyzerNoReturn,
// `__attribute__` spelling only): it applies to functions and to
// function-pointer variables, takes no arguments, and has no effect but
// in the CFG, where a call to such a function ends the path, so
// -Wreturn-type does not fire after it (unlike [[noreturn]] it is not
// checked in the callee and does not change codegen). BDE's
// bsls_annotation.h defines BSLS_ANNOTATION_ANALYZER_NORETURN as it under
// clang, and the reference workload builds with -Werror. The ill-formed
// spellings are in analyzer-noreturn-diagnostics.cpp: an error here
// would suppress the flow warnings this test must not see.
//@ expect: compile-ok
//@ args: -Wall -Werror
//@ strict-diags

void fail() __attribute__((analyzer_noreturn));
void fail2() __attribute__((__analyzer_noreturn__));
void (*handler)() __attribute__((analyzer_noreturn));
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
