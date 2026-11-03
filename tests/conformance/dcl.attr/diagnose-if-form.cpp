// [dcl.attr.grammar]/5 (an implementation attribute): the form of clang's
// diagnose_if. Clang 22 takes 3 or 4 arguments (the 4th a known warning group,
// which suppresses the "from diagnose_if" note); the message, type and group
// must be string literals, the type "error" or "warning", the condition
// contextually convertible to bool; it appertains only to functions.
// Clang 19 wording: diagnose-if-form-19.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 24:62 error "unknown warning group 'no-such'"
//@ diag: 25:31 error "'diagnose_if' attribute takes no more than 4 arguments"
//@ diag: 26:51 error "expected string literal as argument of 'diagnose_if' attribute"
//@ diag: 27:46 error "expected string literal as argument of 'diagnose_if' attribute"
//@ diag: 28:31 error "'diagnose_if' attribute takes at least 3 arguments"
//@ diag: 29:62 error "expected string literal as argument of 'diagnose_if' attribute"
//@ diag: 30:31 error "'diagnose_if' attribute takes at least 3 arguments"
//@ diag: 31:52 error "invalid diagnostic type for 'diagnose_if'; use \"error\" or \"warning\" instead"
//@ diag: 33:44 error "value of type 'P' is not contextually convertible to 'bool'"
//@ diag: 34:24 warning "'diagnose_if' attribute only applies to functions, Objective-C methods, and Objective-C properties"
//@ diag: 35:17 warning "grouped"
//@ diag: 35:24 error "grouped error"
void g(int n) __attribute__((diagnose_if(n, "grouped", "warning", "unused-variable")));
void g2(int n) __attribute__((diagnose_if(n, "grouped error", "error", "unused-variable")));
void g3(int n) __attribute__((diagnose_if(n, "x", "warning", "no-such")));
void g4(int n) __attribute__((diagnose_if(n, "x", "warning", "unused-variable", 1)));
void g5(int n) __attribute__((diagnose_if(n, "x", 3)));
void g6(int n) __attribute__((diagnose_if(n, 4, "error")));
void g7(int n) __attribute__((diagnose_if));
void g8(int n) __attribute__((diagnose_if(n, "x", "warning", 7)));
void g9(int n) __attribute__((diagnose_if(n, "x")));
void g10(int n) __attribute__((diagnose_if(n, "x", "note")));
struct P { int v; };
void g11(int n) __attribute__((diagnose_if(P{}, "x", "warning")));
int g12 __attribute__((diagnose_if(1, "x", "warning")));
void use() { g(1); g2(1); g3(1); g4(1); g9(1); g10(1); }
