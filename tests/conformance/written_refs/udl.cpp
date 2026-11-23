// User-defined literals ([lex.ext], [over.literal]; spec/ast.md
// §18.13.4): the ud-suffix of a user-defined literal has a `ud-suffix`
// entry over the suffix's own bytes naming the literal operator its
// literal calls -- a raw or cooked integer or floating literal, a
// character or string literal of any encoding prefix, a raw string, a
// numeric literal operator template's specialization (`found` the
// template) and a string literal operator template's. Each suffixed
// piece of a concatenation has its own entry naming the one operator.
// A suffix split by a line splice still spans its own bytes, splice
// included.
//@ std: c++20
//@ written-refs: udl.refs
namespace units {
using size_t = decltype(sizeof 0);
struct Len {
  long double v;
};
constexpr Len operator""_m(unsigned long long v) { return {static_cast<long double>(v)}; }
constexpr Len operator""_m(long double v) { return {v}; }
constexpr int operator""_raw(const char *) { return 0; }
template <char... C> constexpr int operator""_t() { return sizeof...(C); }
constexpr char operator""_c(char c) { return c; }
constexpr char32_t operator""_c(char32_t c) { return c; }
constexpr size_t operator""_s(const char *, size_t n) { return n; }
constexpr size_t operator""_s(const char8_t *, size_t n) { return n; }
constexpr size_t operator""_s(const wchar_t *, size_t n) { return n; }
constexpr size_t operator""_s(const char16_t *, size_t n) { return n; }
constexpr size_t operator""_s(const char32_t *, size_t n) { return n; }
struct Fixed {
  char s[4];
  constexpr Fixed(const char (&a)[4]) : s{a[0], a[1], a[2], a[3]} {}
};
template <Fixed F> constexpr int operator""_f() { return F.s[0]; }
} // namespace units

using namespace units;

void use() {
  1_m;
  2.5_m;
  0x10_m;
  42_raw;
  1.5e3_raw;
  123_t;
  'a'_c;
  U'b'_c;
  "x"_s;
  u8"x"_s;
  L"x"_s;
  u"x"_s;
  U"x"_s;
  R"(raw)"_s;
  u8R"d(raw)d"_s;
  "a" "b"_s "c"_s;
  "abc"_f;
  7_\
m;
}
