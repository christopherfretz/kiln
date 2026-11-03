// [expr.type.conv]/2, [dcl.init.general]/16: `T()`, `T{...}` and
// `T(a, b)` for a function type `T` create no object: `cannot create
// object of function type 'T'` at the type, also at instantiation (clang
// `BuildCXXTypeConstructExpr`). `T(a)` is a cast ([expr.cast]) and a
// compound literal `(T){}` an illegal initializer type. clang is the
// oracle.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:3 error "cannot create object of function type 'FF' (aka 'void ()')"
//@ diag: 24:9 error "cannot create object of function type 'FF' (aka 'void ()')"
//@ diag: 25:9 error "cannot create object of function type 'GF' (aka 'int (int)')"
//@ diag: 26:9 error "cannot create object of function type 'FF' (aka 'void ()')"
//@ diag: 27:9 error "cannot create object of function type 'GF' (aka 'int (int)')"
//@ diag: 28:13 error "illegal initializer type 'FF' (aka 'void ()')"
//@ diag: 29:9 error "functional-style cast from 'int' to 'GF' (aka 'int (int)') is not allowed"
//@ diag: 31:37 error "cannot create object of function type 'void ()'"
//@ diag: 32:15 note "in instantiation of function template specialization 't<void ()>' requested here"
//@ diag: 31:48 error "cannot create object of function type 'void ()'"
//@ diag: 31:59 error "cannot create object of function type 'void ()'"
using FF = void();
typedef int GF(int);
void f() {
  FF{};
  (void)FF{};
  (void)GF{1};
  (void)FF();
  (void)GF(1, 2);
  (void)(FF){};
  (void)GF(1);
}
template <class T> void t() { (void)T{}; (void)T(); (void)T(1, 2); }
template void t<FF>();
