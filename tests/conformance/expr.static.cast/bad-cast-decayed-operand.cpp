// [expr.cast]/4, [expr.static.cast]/1, [expr.reinterpret.cast]/1,
// [expr.const.cast]/1: to a non-reference type the operand undergoes the
// array-to-pointer and function-to-pointer conversions first, so a failed
// cast names the decayed type ('int *', 'void (*)(int)'); a const_cast to
// void is no pointer target (WP-327). A class target names the array.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:9 error "static_cast from 'int *' to 'double' is not allowed"
//@ diag: 25:9 error "reinterpret_cast from 'int *' to 'float' is not allowed"
//@ diag: 26:9 error "static_cast from 'const char *' to 'int' is not allowed"
//@ diag: 27:9 error "C-style cast from 'int *' to 'float' is not allowed"
//@ diag: 28:9 error "functional-style cast from 'int *' to 'float' is not allowed"
//@ diag: 29:9 error "static_cast from 'void (*)(int)' to 'int' is not allowed"
//@ diag: 30:9 error "reinterpret_cast from 'void (*)(int)' to 'double' is not allowed"
//@ diag: 31:9 error "static_cast from 'int *' to 'long *' is not allowed"
//@ diag: 33:9 error "static_cast from 'const int *' to 'int *' is not allowed"
//@ diag: 34:9 error "const_cast from 'const int *' to 'long *' is not allowed"
//@ diag: 35:9 error "const_cast from 'void (*)(int)' to 'int *' is not allowed"
//@ diag: 36:3 error "reinterpret_cast from 'int *' to 'void' is not allowed"
//@ diag: 37:3 error "const_cast to 'void', which is not a reference, pointer-to-object, or pointer-to-data-member"
int a[3];
void fn(int);
void g() {
  (void)static_cast<double>(a);
  (void)reinterpret_cast<float>(a);
  (void)static_cast<int>("s");
  (void)(float)a;
  (void)float(a);
  (void)static_cast<int>(fn);
  (void)reinterpret_cast<double>(fn);
  (void)static_cast<long*>(a);
  const int ca[2] = {};
  (void)static_cast<int*>(ca);
  (void)const_cast<long*>(ca);
  (void)const_cast<int*>(fn);
  reinterpret_cast<void>(a);
  const_cast<void>(fn);
}
