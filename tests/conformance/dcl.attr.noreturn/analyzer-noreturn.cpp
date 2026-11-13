// clang's GNU-spelled `analyzer_noreturn` (Attr.td AnalyzerNoReturn,
// `__attribute__` spelling only): it applies to functions and to
// function-pointer variables, takes no arguments, is not checked in the
// callee (no -Winvalid-noreturn) and does not change codegen. In the CFG a
// call of a function so marked ends its path (clang 22
// `CFGBuilder::VisitCallExpr`), so -Wreturn-type does not fire after it;
// a call through a marked function-pointer variable does not (clang 19
// honoured neither, hence the version gate). BDE's bsls_annotation.h
// defines BSLS_ANNOTATION_ANALYZER_NORETURN as it under clang, and the
// code that uses it builds with -Werror, so the attribute must be known
// (not -Wunknown-attributes). The ill-formed spellings are in
// analyzer-noreturn-diagnostics.cpp.
//@ expect: compile-ok
//@ args: -Wall
//@ requires: clang>=22
//@ strict-diags
//@ diag: 30:23 warning "non-void function does not return a value"
void fail() __attribute__((analyzer_noreturn));
void fail2() __attribute__((__analyzer_noreturn__));
void (*handler)() __attribute__((analyzer_noreturn));
struct S {
  void m() __attribute__((analyzer_noreturn));
  static void sm() __attribute__((analyzer_noreturn));
};

int g1(int x) { if (x) return 1; fail(); }
int g2() { fail2(); }
int g3(S &s) { s.m(); }
int g4() { S::sm(); }
int g5() { handler(); }
void h() __attribute__((analyzer_noreturn));
void h() { }
