// [depr.string] (C++03 [conv.array]/2): in C++98 a narrow or wide string
// literal, possibly parenthesized, converts to a pointer to non-const plain
// `char` / `wchar_t`, deprecated, as an exact match (array-to-pointer, then a
// qualification conversion). Found running GCC's C++ testsuite (WP-1467).
//@ std: c++98
//@ strict-diags
//@ diag: 12:11 warning "conversion from string literal to 'char *' is deprecated"
//@ diag: 13:14 warning "conversion from string literal to 'wchar_t *' is deprecated"
//@ diag: 14:11 warning "conversion from string literal to 'char *' is deprecated"
//@ diag: 17:14 warning "conversion from string literal to 'char *' is deprecated"
//@ diag: 21:11 warning "conversion from string literal to 'char *' is deprecated"
char *p = "abc";
wchar_t *w = L"abc";
char *q = ("abc");
const char *ok = "abc";
void f(char *);
void g() { f("x"); }
// An exact match in C++98: chosen over `bool`.
int k(char *) { return 1; }
int k(bool) { return 2; }
int r = k("x");
