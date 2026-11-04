// [namespace.udecl]/1, [lex.ext]/5, [basic.lookup.unqual]/4: a block-scope
// using-declarator may name a literal operator; the unqualified lookup of
// `operator""_t` for a ud-suffix finds it from its point of declaration to
// the end of the block, hiding outer declarations, and it joins an overload
// set with the block's other using-declarators of that name. WP-559.
//@ expect: run
//@ strict-diags
using size_t = decltype(sizeof 0);

namespace udl {
constexpr unsigned long long operator""_t(const char* s, size_t n) {
  unsigned long long h = 5381;
  for (size_t i = 0; i < n; ++i) h = h * 33 + static_cast<unsigned char>(s[i]);
  return h;
}
}  // namespace udl

namespace num {
constexpr unsigned long long operator""_k(unsigned long long v) { return v * 1000; }
constexpr int operator""_t(char c) { return c + 1; }
}  // namespace num

// A namespace-scope declaration of the same suffix, hidden in blocks that
// declare their own.
constexpr unsigned long long operator""_t(const char*, size_t) { return 0; }

constexpr unsigned long long hash(const char* s) {
  size_t n = 0;
  while (s[n]) ++n;
  using udl::operator""_t;
  return operator""_t(s, n);
}

// The shape of a mime-type table: case labels are user-defined literals.
const char* mime(const char* ext) {
  using udl::operator""_t;
  switch (hash(ext)) {
  case "css"_t: return "text/css";
  case "html"_t: return "text/html";
  case "js"_t: return "text/javascript";
  default: return "application/octet-stream";
  }
}

constexpr bool streq(const char* a, const char* b) {
  while (*a && *a == *b) { ++a; ++b; }
  return *a == *b;
}

// Two suffixes from two namespaces, and two declarators of one suffix
// (`_t` from udl and from num) forming one overload set.
constexpr unsigned long long mixed() {
  using udl::operator""_t;
  using num::operator""_k;
  using num::operator""_t;
  return "a"_t + 2_k + 'b'_t;
}
static_assert(mixed() == 5381 * 33 + 'a' + 2000 + 'b' + 1);

// The outer declaration is found until an inner block's declarator hides it.
constexpr bool hides() {
  bool outer = ""_t == 0 && "x"_t == 0;
  {
    using udl::operator""_t;
    if ("x"_t != 5381 * 33 + 'x') return false;
  }
  return outer && "x"_t == 0;
}
static_assert(hides());

constexpr bool in_constexpr() {
  using udl::operator""_t;
  static_assert("a"_t == 5381 * 33 + 'a');
  constexpr auto h = "ab"_t;
  return h == hash("ab");
}
static_assert(in_constexpr());

// A non-dependent declarator in a function template.
template <class T>
constexpr T tmpl(T x) {
  using udl::operator""_t;
  return x + static_cast<T>("a"_t);
}
static_assert(tmpl(1ull) == 5381 * 33 + 'a' + 1);

int main() {
  if (!streq(mime("css"), "text/css")) return 1;
  if (!streq(mime("js"), "text/javascript")) return 2;
  if (!streq(mime("png"), "application/octet-stream")) return 3;
  if (tmpl<unsigned long long>(0) != hash("a")) return 4;
  return mixed() == 5381 * 33 + 'a' + 2000 + 'b' + 1 ? 0 : 5;
}
