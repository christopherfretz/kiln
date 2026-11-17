// [class.static.data]/4: from C++11 an in-class initializer of a `static
// const` member of floating or complex floating type needs `constexpr`
// (-Wstatic-float-init, an error by default) and other literal types do too;
// both are reported at the member's name. Verified against clang 19 and 22.
//@ expect: compile-fail
//@ args: -std=gnu++11
//@ diag: 11:32 error "in-class initializer for static data member of type 'const _Complex double' requires 'constexpr' specifier"
//@ diag: 12:23 error "in-class initializer for static data member of type 'const double' requires 'constexpr' specifier"
//@ diag: 13:29 error "in-class initializer for static data member of type 'const _Complex int' requires 'constexpr' specifier"
struct A {
  static const _Complex double x = 1.0;
  static const double d = 2.0;
  static const _Complex int ci = 1;
  static constexpr _Complex double y = 1.0;
};
