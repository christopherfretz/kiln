//@ expect: compile-fail
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ args: -freflection
//@ strict-diags
//@ diag: 22:9 error "an annotation can appertain only to a declaration"
//@ diag: 23:10 error "an annotation can appertain only to a declaration"
//@ diag: 25:25 error "an annotation can appertain only to a declaration"
//@ diag: 26:28 error "an annotation can appertain only to a declaration"
//@ diag: 31:5 error "an annotation cannot appertain to an unnamed bit-field"
// [dcl.attr.annotation]/1: an annotation applies to a declaration (or a
// base-specifier), never to a type. The attribute-specifier-seq after a
// ref-qualifier appertains to the reference type ([dcl.ref]/1), the one
// after a bracket of a new-declarator to that array type, the outermost
// one included ([expr.new]/1; GCC 16.1 only warns that it ignores that
// one). An unnamed bit-field is not a member ([class.bit]/2). The
// annotations on the declarator-ids, and on a named bit-field, are the
// accepted side. Found running GCC's C++ testsuite.

int f;
int &r1 [[=1]] = f;
int &&r2 [[=2]] = 0;
int & [[=3]] r3 = f;
int && [[=4]] r4 = 0;
void g(int n) {
  auto p = new int[n] [[=5]];
  auto q = new int[n][4] [[=6]];
  delete[] p;
  delete[] q;
}
struct B {
  [[=7]] int : 0;
  [[=9]] int named : 3;
};
