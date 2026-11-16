// [depr.string], [diff.cpp03.conv]: C++11 removed the string literal to
// `char*` conversion; clang keeps it as an extension ("ISO C++11 does not
// allow", -Wwritable-strings), ranked worse than any other conversion
// sequence, an ellipsis one included. Found running GCC's C++ testsuite
// (WP-1467).
//@ std: c++17
//@ strict-diags
//@ diag: 13:11 warning "ISO C++11 does not allow conversion from string literal to 'char *'"
//@ diag: 14:14 warning "ISO C++11 does not allow conversion from string literal to 'wchar_t *'"
//@ diag: 15:11 warning "ISO C++11 does not allow conversion from string literal to 'char *'"
//@ diag: 17:14 warning "ISO C++11 does not allow conversion from string literal to 'char *'"
//@ diag: 26:7 warning "ISO C++11 does not allow conversion from string literal to 'char *'"
char *p = "abc";
wchar_t *w = L"abc";
char *q = ("abc");
void f(char *);
void g() { f("x"); }
// Since C++11 worse than any other conversion, an ellipsis included.
constexpr int k(char *) { return 1; }
constexpr int k(bool) { return 2; }
static_assert(k("x") == 2, "");
constexpr int h(char *) { return 1; }
constexpr int h(...) { return 2; }
static_assert(h("x") == 2, "");
struct X { X(char *) {} };
X x = "q";
