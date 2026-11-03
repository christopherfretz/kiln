// [dcl.attr.grammar]/5 (an implementation attribute): the form of clang's
// diagnose_if. Clang 19 takes exactly 3 arguments; the message and type
// must be string literals, the type "error" or "warning", the condition
// contextually convertible to bool; it appertains only to functions.
// Clang 22 wording: diagnose-if-form.cpp.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 21:30 error "'diagnose_if' attribute requires exactly 3 arguments"
//@ diag: 22:31 error "'diagnose_if' attribute requires exactly 3 arguments"
//@ diag: 23:31 error "'diagnose_if' attribute requires exactly 3 arguments"
//@ diag: 24:31 error "'diagnose_if' attribute requires exactly 3 arguments"
//@ diag: 25:51 error "expected string literal as argument of 'diagnose_if' attribute"
//@ diag: 26:46 error "expected string literal as argument of 'diagnose_if' attribute"
//@ diag: 27:31 error "'diagnose_if' attribute requires exactly 3 arguments"
//@ diag: 28:31 error "'diagnose_if' attribute requires exactly 3 arguments"
//@ diag: 29:31 error "'diagnose_if' attribute requires exactly 3 arguments"
//@ diag: 30:52 error "invalid diagnostic type for 'diagnose_if'; use \"error\" or \"warning\" instead"
//@ diag: 32:44 error "value of type 'P' is not contextually convertible to 'bool'"
//@ diag: 33:24 warning "'diagnose_if' attribute only applies to functions, Objective-C methods, and Objective-C properties"
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
