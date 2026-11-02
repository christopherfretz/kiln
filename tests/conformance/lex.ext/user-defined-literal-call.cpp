// [lex.ext]: user-defined literals call literal operators in the cooked,
// raw, numeric-template, string, class-NTTP string template and GNU forms
//@ expect: compile-ok
//@ strict-diags
//@ diag: 38:43 warning "string literal operator templates are a GNU extension"
//@ diag: 42:18 warning "user-defined literal suffixes not starting with '_' are reserved"
typedef decltype(sizeof 0) size_t;
namespace lit {
struct sv { const char* p; size_t n; };
constexpr sv operator""_sv(const char* p, size_t n) { return {p, n}; }
constexpr unsigned long long operator""_k(unsigned long long v) { return v * 1000; }
constexpr long double operator""_h(long double v) { return v * 100; }
constexpr int operator""_c(char c) { return c + 1; }
constexpr int operator""_w(char16_t c) { return c + 2; }
constexpr size_t operator""_r(const char* s) { size_t n = 0; while (s[n]) ++n; return n; }
template <char... Cs> constexpr int operator""_t() { return (0 + ... + (Cs - '0')); }
struct fs { char d[8]; constexpr fs(const char (&s)[4]) : d{s[0], s[1], s[2], 0} {} };
template <fs F> constexpr char operator""_f() { return F.d[1]; }
constexpr size_t operator""_u(const char16_t*, size_t n) { return n; }
}
using namespace lit;
static_assert("abc"_sv.n == 3);
static_assert("ab" "cd"_sv.n == 4);
static_assert(R"(x)"_sv.n == 1);
static_assert(5_k == 5000);
static_assert(0x10_k == 16000);
static_assert(1.5_h == 150.0L);
static_assert('a'_c == 'b');
static_assert(u'a'_w == 'c');
static_assert(12345_r == 5);
static_assert(1.25e3_r == 6);
static_assert(123_t == 6);
static_assert("xyz"_f == 'y');
static_assert(u"hello"_u == 5);
template <class T> constexpr T twice() { return 2_k + T(1); }
static_assert(twice<int>() == 2001);
// GNU string literal operator template (clang ExtWarn, off by default).
template <class C, C... Cs> constexpr int operator""_g() { return sizeof...(Cs); }
static_assert("four"_g == 4);
// A reserved suffix spelled like a standard one ([usrlit.suffix]) is
// still callable.
constexpr size_t operator""sv(const char*, size_t n) { return n; } // reserved
static_assert("ab"sv == 2);
