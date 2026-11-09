//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
// [dcl.constexpr]/6, clang `isPotentialConstantExpr`: a call's direct
// callee is not a value the body reads, so naming a consteval function
// or a builtin as the callee never makes a body "never constant"
// (WP-933, reduced from folly::coro xlog.h and rapidhash.h).
using size_t = decltype(sizeof 0);

constexpr unsigned long hash(const char* p, size_t len) {
  if (__builtin_expect(static_cast<bool>(len <= 16), 1))
    return len ? static_cast<unsigned char>(p[0]) : 0;
  return 2;
}
static_assert(hash("ab", 2) == 'a');

consteval bool is_sep(char c) { return c == '/'; }

namespace detail {
consteval const char* strip_rec(const char* s, size_t i, bool m);
consteval const char* strip_found(const char* s, size_t i) {
  return (s[i] == '\0') ? strip_rec(s, 0, true) : strip_rec(s, i + 1, false);
}
consteval const char* strip_rec(const char* s, size_t i, bool m) {
  return s[i] == '\0' ? s : is_sep(s[i]) ? strip_rec(s + i + 1, 0, m) : strip_rec(s, i + 1, m);
}
} // namespace detail

consteval const char* strip(const char* s) {
  return detail::strip_rec(s, 0, true);
}
static_assert(*strip("dir/file.cpp") == 'f');
static_assert(*detail::strip_found("x", 1) == 'x');

int main() { return hash("a", 1) == 'a' ? 0 : 1; }
