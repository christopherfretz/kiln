// GNU _Complex under -pedantic (clang 19): the C99 extension diagnostic on
// every `_Complex` keyword, in any declaration-specifier context, and the
// C2y extension for `++`/`--` on any complex object (WP-211).
//@ expect: compile-ok
//@ args: -pedantic
//@ strict-diags
//@ diag: 23:8 warning "'_Complex' is a C99 extension"
//@ diag: 23:25 warning "'_Complex' is a C99 extension"
//@ diag: 23:47 warning "'++' on an object of complex type is a C2y extension"
//@ diag: 23:53 warning "'--' on an object of complex type is a C2y extension"
//@ diag: 23:58 warning "'++' on an object of complex type is a C2y extension"
//@ diag: 23:62 warning "'--' on an object of complex type is a C2y extension"
//@ diag: 25:3 warning "'_Complex' is a C99 extension"
//@ diag: 26:17 warning "'_Complex' is a C99 extension"
//@ diag: 26:50 warning "'_Complex' is a C99 extension"
//@ diag: 27:22 warning "'_Complex' is a C99 extension"
//@ diag: 28:17 warning "'_Complex' is a C99 extension"
//@ diag: 28:56 warning "'++' on an object of complex type is a C2y extension"
//@ diag: 28:71 warning "'_Complex' is a C99 extension"
//@ diag: 29:11 warning "'_Complex' is a C99 extension"
//@ diag: 30:12 warning "'_Complex' is a C99 extension"
//@ diag: 30:35 warning "'_Complex' is a C99 extension"
void f(_Complex int& i, _Complex double& d) { ++i; d--; i++; --d; }
template <class T> struct W { T t; };
W<_Complex float> w;
auto s = sizeof(_Complex double) + sizeof(double _Complex);
double c = __real__ (_Complex float)1;
void g() { for (_Complex double x = 0; __real__ x < 2; ++x) {} (void)(_Complex int)(2); }
int (*fp)(_Complex long double);
struct S { _Complex unsigned u; S(_Complex short); };
