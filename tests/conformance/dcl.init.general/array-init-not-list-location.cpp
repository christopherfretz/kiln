// [dcl.init.general]/16.5: an array initialized from an expression that is
// not a braced-init-list (and not a string literal for a character array) is
// ill-formed. clang reports "array initializer must be an initializer list"
// at the declarator's name for a variable, and at the initializer for a
// default member initializer; the suggested literal follows the element type
// (plain/signed/unsigned char: string literal; unqualified wchar_t, char16_t,
// char32_t: wide string literal; char8_t and a qualified wide type: none).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 30:5 error "array initializer must be an initializer list"
//@ diag: 31:5 error "array initializer must be an initializer list"
//@ diag: 32:5 error "array initializer must be an initializer list"
//@ diag: 34:6 error "array initializer must be an initializer list or string literal"
//@ diag: 35:5 error "array initializer must be an initializer list"
//@ diag: 36:10 error "cannot initialize an array element of type 'int' with an lvalue of type 'int[2]'"
//@ diag: 37:16 error "array initializer must be an initializer list"
//@ diag: 38:23 error "array initializer must be an initializer list"
//@ diag: 39:37 error "array initializer must be an initializer list"
//@ diag: 39:27 note "in instantiation of default member initializer 'V<int[2]>::m' requested here"
//@ diag: 40:11 note "in evaluation of exception specification for 'V<int[2]>::V' needed here"
//@ diag: 41:6 error "array initializer must be an initializer list or string literal"
//@ diag: 42:9 error "array initializer must be an initializer list or wide string literal"
//@ diag: 43:9 error "array initializer must be an initializer list"
//@ diag: 44:10 error "array initializer must be an initializer list or wide string literal"
//@ diag: 45:10 error "array initializer must be an initializer list or wide string literal"
//@ diag: 46:15 error "array initializer must be an initializer list"
//@ diag: 47:15 error "array initializer must be an initializer list or string literal"
//@ diag: 48:13 error "array initializer must be an initializer list or string literal"
int g();
int a[2] = 1;
int b[2] = g();
int c[2] = (1);
int d[2](1);
char e[3] = 1 + 2;
int x[2] = a;
int y[2](a);
void f() { int l[2] = 3; int m[2](g()); }
struct T { int m[2] = 1; int n[2]; T() : n{} {} };
template <class X> struct V { X m = 1; };
V<int[2]> v;
char str[3] = 'x';
wchar_t ws[3] = 1;
char8_t a8[2] = 1;
char16_t a16[2] = 1;
char32_t a32[2] = 1;
const wchar_t aw[2] = 1;
unsigned char au[2] = 1;
signed char as[2] = 1;
