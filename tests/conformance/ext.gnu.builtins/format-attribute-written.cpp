// A written GNU format(archetype, F, A) attribute (WP-186; GNU [dcl.attr]
// extension, clang handleFormatAttr): exactly three arguments, the
// archetype an identifier clang supports (__x__ normalized), F naming a
// parameter of pointer-to-character type (index 1 of an implicit object
// member function is `this` and cannot be the format), A zero or the
// position of the `...` (GCC requires a variadic function), strftime
// with A = 0. Calls to a printf-format function are checked like the
// builtins' (member calls count `this`). Verified against clang 22 and 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 36:46 error "'format' attribute parameter 2 is out of bounds"
//@ diag: 37:46 error "'format' attribute parameter 2 is out of bounds"
//@ diag: 38:38 error "format argument not a string type"
//@ diag: 39:46 warning "'format' attribute argument not supported: foo"
//@ diag: 40:6 warning "GCC requires a function with the 'format' attribute to be variadic"
//@ diag: 41:46 error "'format' attribute parameter 3 is out of bounds"
//@ diag: 42:46 error "'format' attribute requires exactly 3 arguments"
//@ diag: 43:46 error "'format' attribute requires parameter 1 to be an identifier"
//@ diag: 44:46 error "'format' attribute requires parameter 2 to be an integer constant"
//@ diag: 45:47 error "'format' attribute parameter 3 is out of bounds"
//@ diag: 46:50 error "format argument not a string type"
//@ diag: 47:47 error "strftime format attribute requires 3rd parameter to be 0"
//@ diag: 48:26 warning "'format' attribute only applies to Objective-C methods, blocks, and non-K&R-style functions"
//@ diag: 49:54 error "format attribute cannot specify the implicit this argument as the format string"
//@ diag: 70:12 warning "more '%' conversions than data arguments"
//@ diag: 71:13 warning "format specifies type 'char *' but the argument has type 'double'"
//@ diag: 72:16 warning "data argument not used by format string"
//@ diag: 73:26 warning "more '%' conversions than data arguments"
//@ diag: 75:12 warning "more '%' conversions than data arguments"
//@ diag: 76:13 warning "format specifies type 'char *' but the argument has type 'int'"
//@ diag: 77:14 warning "format specifies type 'void *' but the argument has type 'int'"
//@ diag: 78:15 warning "format specifies type 'char *' but the argument has type 'int'"
//@ diag: 79:15 warning "format specifies type 'int' but the argument has type 'const char *'"
//@ diag: 80:7 warning "format string is not a string literal (potentially insecure)"
//@ diag: 80:7 note "treat the string as an argument to avoid this"
void bad1(const char *f, ...) __attribute__((format(printf, 2, 3)));
void bad2(const char *f, ...) __attribute__((format(printf, 0, 2)));
void bad3(int f, ...) __attribute__((format(printf, 1, 2)));
void bad4(const char *f, ...) __attribute__((format(foo, 1, 2)));
void bad5(const char *f, int) __attribute__((format(printf, 1, 2)));
void bad6(const char *f, ...) __attribute__((format(printf, 1, 1)));
void bad7(const char *f, ...) __attribute__((format(printf, 1)));
void bad8(const char *f, ...) __attribute__((format(1, 1, 2)));
void bad9(const char *f, ...) __attribute__((format(printf, "x", 2)));
void bad10(const char *f, ...) __attribute__((format(printf, 1, 3)));
void bad11(const wchar_t *f, ...) __attribute__((format(printf, 1, 2)));
void bad12(const char *f, ...) __attribute__((format(strftime, 1, 2)));
int bad13 __attribute__((format(printf, 1, 2)));
struct B { void m(const char *f, ...) __attribute__((format(printf, 1, 2))); };

void ok1(const char *f, ...) __attribute__((__format__(__printf__, 1, 2)));
[[gnu::format(printf, 1, 2)]] void ok3(const char *f, ...);
void ok4(unsigned char *f, ...) __attribute__((format(printf, 1, 2)));
void ok5(const char *f, __builtin_va_list) __attribute__((format(printf, 1, 0)));
void ok6(const char *f, ...) __attribute__((format(gcc_diag, 1, 2)));
void ok7(const char *f, ...) __attribute__((format(scanf, 1, 2)));
void ok8(const char *f) __attribute__((format(strftime, 1, 0)));

struct M {
  void m(const char *f, ...) __attribute__((format(printf, 2, 3)));
  static void s(const char *f, ...) __attribute__((format(printf, 1, 2)));
};
template <class T> struct L {
  void log(const char *f, ...) __attribute__((format(printf, 2, 3)));
};
template <class T> void tf(T v, const char *f, ...) __attribute__((format(printf, 2, 3)));
void ok1(const char *f, ...);

void test(M m, L<int> l, const char *s) {
  ok1("%d %d", 1);
  ok1("%s", 1.0);
  ok3("%d", 1, 2);
  ok4((unsigned char *)"%d");
  ok6("%d");
  m.m("%d %d", 1);
  m.m("%s", 1);
  M::s("%p", 1);
  l.log("%s", 1);
  tf(1, "%d", "x");
  m.m(s);
  M::s(s, 1);
}
