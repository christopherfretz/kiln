// User-defined literals ([lex.ext]) calling literal operators
// ([over.literal]): the cooked, raw, template and string forms, and the
// Itanium `li <source-name>` operator names.
typedef unsigned long size_t;
namespace lit {
struct sv { const char* p; size_t n; };
sv operator""_sv(const char* p, size_t n) { return {p, n}; }
unsigned long long operator"" _k(unsigned long long v) { return v * 1000; }
long double operator""_h(long double v) { return v * 100; }
int operator""_c(char c) { return c + 1; }
int operator""_w(char32_t c) { return static_cast<int>(c) + 2; }
size_t operator""_r(const char* s) { size_t n = 0; while (s[n]) ++n; return n; }
template <char... Cs> int operator""_t() { return (0 + ... + (Cs - '0')); }
size_t operator""_u(const char16_t*, size_t n) { return n; }
}
using namespace lit;
template <class T> T twice() { return T(2_k) + T(1); }
int use() {
  sv a = "abc"_sv;
  unsigned long long k = 5_k + 0x10_k;
  long double h = 1.5_h;
  int c = 'a'_c + U'b'_w;
  size_t r = 12345_r + 1.25e3_r;
  int t = 123_t;
  size_t u = u"hello"_u;
  return static_cast<int>(a.n + k + h + c + r + t + u) + twice<int>();
}
