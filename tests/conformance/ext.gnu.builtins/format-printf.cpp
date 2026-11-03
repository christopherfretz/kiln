// The implicit format(printf, F, A) attribute of __builtin_sprintf (2, 3),
// __builtin_snprintf (3, 4) and __builtin_vsnprintf (3, 0) and the
// -Wformat checks of a literal format string (GNU [dcl.attr] extension;
// clang 19 CheckPrintfHandler): the argument type a conversion
// specifies (signedness and the integral promotions ignored), `*` of
// type int, more conversions than arguments (the walk stops there) and
// unused arguments. Verified against clang 19.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 60:32 warning "format specifies type 'int' but the argument has type 'long'"
//@ diag: 61:38 warning "format specifies type 'int' but the argument has type 'double'"
//@ diag: 61:41 warning "format specifies type 'double' but the argument has type 'int'"
//@ diag: 61:44 warning "format specifies type 'int' but the argument has type 'long'"
//@ diag: 62:38 warning "format specifies type 'char *' but the argument has type 'int'"
//@ diag: 62:41 warning "format specifies type 'char *' but the argument has type 'std::nullptr_t'"
//@ diag: 62:50 warning "format specifies type 'char *' but the argument has type 'const wchar_t *'"
//@ diag: 63:39 warning "format specifies type 'void *' but the argument has type 'int'"
//@ diag: 63:42 warning "format specifies type 'unsigned int' but the argument has type 'int *'"
//@ diag: 63:46 warning "format specifies type 'long' but the argument has type 'int *'"
//@ diag: 64:47 warning "format specifies type 'size_t' (aka 'unsigned long') but the argument has type 'unsigned long long'"
//@ diag: 64:52 warning "format specifies type 'ssize_t' (aka 'long') but the argument has type 'double'"
//@ diag: 64:55 warning "format specifies type 'long long' but the argument has type 'long'"
//@ diag: 64:58 warning "format specifies type 'unsigned long long' but the argument has type 'double'"
//@ diag: 65:36 warning "format specifies type 'long' but the argument has underlying type 'unsigned int'"
//@ diag: 65:40 warning "format specifies type 'int' but the argument has type '__int128'"
//@ diag: 66:40 warning "format specifies type 'double' but the argument has type 'long double'"
//@ diag: 66:44 warning "format specifies type 'double' but the argument has type 'long double'"
//@ diag: 67:46 warning "format specifies type 'long' but the argument has type 'int'"
//@ diag: 68:29 warning "field width should have type 'int', but argument has type 'long'"
//@ diag: 71:31 warning "more '%' conversions than data arguments"
//@ diag: 72:28 warning "more '%' conversions than data arguments"
//@ diag: 73:31 warning "more '%' conversions than data arguments"
//@ diag: 74:33 warning "more '%' conversions than data arguments"
//@ diag: 75:35 warning "data argument not used by format string"
//@ diag: 76:36 warning "format specifies type 'long' but the argument has type 'int'"
//@ diag: 76:42 warning "data argument not used by format string"
//@ diag: 77:44 warning "data argument not used by format string"
//@ diag: 79:64 warning "format specifies type 'int' but the argument has type 'double'"
//@ diag: 81:15 note "in instantiation of function template specialization 'tp<double>' requested here"
//@ requires: clang<22
// clang 19 spelling; format-printf-clang22.cpp has clang 22's (WP-394).
#include <stddef.h>
char buf[64];
enum E { e0 };
enum class EC : long { a };
enum EU : unsigned char { u0 };
void (*fp)();
void ok(int i, unsigned u, long l, unsigned long ul, long long ll, size_t z, const char *s, char c, void *p,
        double d, float fl, short sh, bool b, wchar_t w, const unsigned char *ucs, char arr[4]) {
  __builtin_sprintf(buf, "%d %u %ld %lld %zu %s %c %p %f %g %x", i, u, l, ll, z, s, c, p, d, fl, u);
  __builtin_sprintf(buf, "%i %o %X %e %E %G %a %F %%", i, u, u, d, d, d, d, fl);
  __builtin_sprintf(buf, "%u %x %zu %ld %lu", i, i, l, ul, sizeof(int));
  __builtin_sprintf(buf, "%d %d %d %c %c %x", sh, c, b, w, 65, b);
  __builtin_sprintf(buf, "%d %d %s %s %s", e0, u0, ucs, arr, "lit");
  __builtin_sprintf(buf, "%p %p %p %d", fp, nullptr, s, EC::a);
  __builtin_sprintf(buf, "%+08.3f %-5d %#x % d %*d %.*f", d, i, u, i, i, i, i, d);
  __builtin_snprintf(buf, sizeof buf, "%s=%d", s, i);
}
void bad(int i, long l, unsigned long long ull, long double ld, int *ip, __int128 q, double d) {
  __builtin_sprintf(buf, "%d", l);
  __builtin_sprintf(buf, "%d %f %c", d, i, l);
  __builtin_sprintf(buf, "%s %s %s", i, nullptr, L"w");
  __builtin_sprintf(buf, "%p %u %ld", 0, ip, ip);
  __builtin_sprintf(buf, "%zu %zd %lld %llx", ull, d, l, d);
  __builtin_sprintf(buf, "%ld %d", e0, q);
  __builtin_sprintf(buf, "%f %+08.3f", ld, ld);
  __builtin_snprintf(buf, sizeof buf, "%ld", i);
  __builtin_sprintf(buf, "%*d %5.*f", l, 1, l, 1.0);
}
void count(int i) {
  __builtin_sprintf(buf, "%d %d", i);
  __builtin_sprintf(buf, "%d %");
  __builtin_sprintf(buf, "%*.*d", 1, 2);
  __builtin_sprintf(buf, "%d" "%d" "%d", 1);
  __builtin_sprintf(buf, "%d", i, i);
  __builtin_sprintf(buf, "%ld %d", 1, 2, 3);
  __builtin_sprintf(buf, "no conversions", i);
}
template <class T> void tp(T t) { __builtin_sprintf(buf, "%d", t); }
template void tp<int>(int);
template void tp<double>(double);
