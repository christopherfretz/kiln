// [basic.scope.block]/2, [dcl.stc]/5, [basic.link]/6-7: a block-scope
// redeclaration of a variable or parameter follows clang's MergeVarDecl
// order: a different type ("redefinition"/"redeclaration ... with a
// different type", skipped when the earlier declarator is invalid), then
// static after a declaration with linkage, non-static after static, extern
// after a declaration without linkage (a different kind of symbol when an
// outer declaration with linkage is also found), non-extern after extern,
// then a plain redefinition. A qualified declarator diagnosed in a block
// still declares its name there, with linkage. WP-235.
//@ expect: compile-fail
//@ strict-diags
// Verified against clang 19 (WP-235).
//@ diag: 46:23 error "redefinition of 'p' with a different type: 'long' vs 'int'"
//@ diag: 46:13 note "previous definition is here"
//@ diag: 47:39 error "redefinition of 's' with a different type: 'long' vs 'int'"
//@ diag: 47:24 note "previous definition is here"
//@ diag: 48:31 error "non-static declaration of 'a' follows static declaration"
//@ diag: 48:24 note "previous definition is here"
//@ diag: 49:31 error "non-extern declaration of 'e' follows extern declaration"
//@ diag: 49:24 note "previous declaration is here"
//@ diag: 50:34 error "extern declaration of 'q' follows non-extern declaration"
//@ diag: 50:17 note "previous definition is here"
//@ diag: 51:39 error "static declaration of 'e2' follows non-static declaration"
//@ diag: 51:24 note "previous declaration is here"
//@ diag: 52:40 error "redeclaration of 'e3' with a different type: 'long' vs 'int'"
//@ diag: 52:24 note "previous declaration is here"
//@ diag: 54:29 error "redefinition of 'arr' with a different type: 'int[3]' vs 'int[2]'"
//@ diag: 54:17 note "previous definition is here"
//@ diag: 55:32 error "redefinition of 'a' as different kind of symbol"
//@ diag: 55:18 note "previous definition is here"
//@ diag: 57:12 error "variable has incomplete type 'Q'"
//@ diag: 43:8 note "forward declaration of 'Q'"
//@ diag: 58:7 error "non-static declaration of 'b' follows static declaration"
//@ diag: 57:12 note "previous definition is here"
//@ diag: 62:7 error "redefinition of 'c' with a different type: 'int' vs 'T'"
//@ diag: 61:12 note "previous definition is here"
//@ diag: 65:14 error "definition or redeclaration of 'a' cannot name the global scope"
//@ diag: 66:8 error "non-static declaration of 'a' follows static declaration"
//@ diag: 65:14 note "previous definition is here"
//@ diag: 69:7 error "definition or redeclaration of 'a' cannot name the global scope"
//@ diag: 70:7 error "non-extern declaration of 'a' follows extern declaration"
//@ diag: 69:7 note "previous definition is here"
struct Q;
struct T { T(); };
int a;
void f1(int p) { long p; }
void f2() { static int s; static long s; }
void f3() { static int a; int a; }
void f4() { extern int e; int e; }
void f5() { int q[2]; extern int q[2]; }
void f6() { extern int e2; static int e2; }
void f7() { extern int e3; extern long e3; }
void f8() { extern int e4; extern int e4; }
void f9() { int arr[2]; int arr[3]; }
void f10() { int a; extern int a; }
void f11() {
  static Q(b);
  int b;
}
void f12() {
  static T(c);
  int c;
}
void f13() {
  static T(::a);
  long a = 1;
}
void f14() {
  T(::a);
  int a;
}
