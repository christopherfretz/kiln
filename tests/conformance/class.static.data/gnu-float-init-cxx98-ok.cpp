// [class.static.data]/4 (C++98: only a const integral or enumeration member
// takes an in-class initializer): a `static const` member of floating or
// complex floating type with an in-class initializer that folds to a constant
// is a GNU extension at -std=gnu++98 and -std=c++98, accepted with
// -Wgnu-static-float-init as clang and GCC do (found running GCC's C++
// testsuite). The value is usable as clang allows: folded in an array bound
// (a further extension) and read at run time. Verified against clang 19 and 22.
//@ expect: run
//@ strict-diags
//@ args: -std=gnu++98
//@ diag: 16:32 warning "in-class initializer for static data member of type 'const _Complex double' is a GNU extension"
//@ diag: 17:23 warning "in-class initializer for static data member of type 'const double' is a GNU extension"
//@ diag: 18:31 warning "in-class initializer for static data member of type 'const volatile float' is a GNU extension"
//@ diag: 22:5 warning "variable length array folded to constant array as an extension"
struct A {
  static const _Complex double x = 1.0 + 2;
  static const double d = 2.5;
  static const volatile float v = 4.0f;
};
const double A::d;
const volatile float A::v;
int arr[(int)A::d];
int main() {
  if (__real__ A::x != 3.0 || __imag__ A::x != 0.0) return 1;
  if (A::v != 4.0f || sizeof arr != 2 * sizeof(int)) return 2;
  const double *p = &A::d;
  return *p == 2.5 ? 0 : 3;
}
