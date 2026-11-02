// [stmt.ambig]/1, [dcl.type.simple]/2, [dcl.ref]/5: at block scope an
// undeclared name followed by a ptr-operator (`u5 &r;`, `u5 *p;`) is
// ambiguous; the declarator decides, as clang's `ANK_Unresolved` in
// `isCXXSimpleDeclaration`. When it is a declaration, "unknown type name" is
// reported and the type recovers as `int` (so an uninitialized reference
// still requires an initializer); `u5 & r5 + 1;` and `u5 & i;` with a
// declared `i` stay expressions. `const u7 &c;` is a declaration with an
// unknown type name, not a missing type specifier. After a decl-specifier
// that names no type, later declarators of the group are skipped (clang's
// `ParseDeclGroup`), so `u5 &x1, &x2;` reports once.
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 51:3 error "unknown type name 'u5'"
//@ diag: 51:7 error "declaration of reference variable 'r1' requires an initializer"
//@ diag: 52:3 error "unknown type name 'u5'"
//@ diag: 53:3 error "unknown type name 'u5'"
//@ diag: 53:8 error "declaration of reference variable 'r3' requires an initializer"
//@ diag: 54:3 error "unknown type name 'u5'"
//@ diag: 55:3 error "use of undeclared identifier 'u5'"
//@ diag: 55:8 error "use of undeclared identifier 'r5'"
//@ diag: 56:3 error "unknown type name 'u5'"
//@ diag: 56:7 error "declaration of reference variable 'r6' requires an initializer"
//@ diag: 57:3 error "unknown type name 'u5'"
//@ diag: 57:8 error "declaration of reference variable 'i' requires an initializer"
//@ diag: 58:3 error "unknown type name 'u5'"
//@ diag: 59:3 error "unknown type name 'u5'"
//@ diag: 59:9 error "declaration of reference variable 'r8' requires an initializer"
//@ diag: 60:3 error "unknown type name 'u5'"
//@ diag: 60:9 error "'r9' declared as array of references of type 'int &'"
//@ diag: 61:9 error "unknown type name 'u7'"
//@ diag: 61:13 error "declaration of reference variable 'c1' requires an initializer"
//@ diag: 62:9 error "unknown type name 'u7'"
//@ diag: 63:9 error "unknown type name 'u7'"
//@ diag: 64:12 error "unknown type name 'u7'"
//@ diag: 65:3 error "unknown type name 'u5'"
//@ diag: 66:10 error "unknown type name 'u5'"
//@ diag: 66:14 error "declaration of reference variable 'r11' requires an initializer"
//@ diag: 67:3 error "unknown type name 'u5'"
//@ diag: 70:4 error "no type named 'c' in 'S'"
//@ diag: 70:7 error "declaration of reference variable 'r1' requires an initializer"
//@ diag: 71:1 error "unknown type name 'u1'"
//@ diag: 71:5 error "declaration of reference variable 'r3' requires an initializer"
//@ diag: 72:1 error "unknown type name 'u2'"
//@ diag: 74:3 error "unknown type name 'u5'"
//@ diag: 74:7 error "declaration of reference variable 'x1' requires an initializer"
//@ diag: 75:6 error "no type named 'c' in 'S'"
//@ diag: 75:9 error "declaration of reference variable 'x3' requires an initializer"
int i;
void h1() {
  u5 &r1;
  u5 *r2;
  u5 &&r3;
  u5 &r4 = i;
  u5 & r5 + 1;
  u5 &r6, &r7;
  u5 & i;
  u5 * i;
  u5 & (r8);
  u5 &r9[2];
  const u7 &c1;
  const u7 c2;
  const u7 *c3;
  volatile u7 &c4 = i;
  u5 &r10 = 1;
  static u5 &r11;
  u5 &f();
}
struct S {};
S::c &r1, &r2;
u1 &r3, &r4, r5[];
u2 a1, a2[];
void h2() {
  u5 &x1, &x2;
  S::c &x3, &x4;
}
