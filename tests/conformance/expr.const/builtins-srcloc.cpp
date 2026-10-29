// [support.srcloc.cons], [dcl.fct.default]/9: __builtin_LINE, _COLUMN,
// _FILE, _FILE_NAME, _FUNCTION and __builtin_source_location() are constant
// expressions; in a default argument they take the location of the call
// that uses the default (the outermost default argument wins).
//@ expect: compile-ok
//@ strict-diags
namespace std {
struct source_location {
  struct __impl { const char *_M_file_name; const char *_M_function_name; unsigned _M_line; unsigned _M_column; };
};
} // namespace std
using impl = std::source_location::__impl;
constexpr unsigned ln(unsigned l = __builtin_LINE()) { return l; }
constexpr unsigned cl(unsigned c = __builtin_COLUMN()) { return c; }
constexpr unsigned ln2(unsigned l = ln()) { return l; }
constexpr const char *fnm(const char *f = __builtin_FUNCTION()) { return f; }
constexpr const impl *sl(const impl *p = __builtin_source_location()) { return p; }
constexpr bool eq(const char *a, const char *b) {
  while (*a == *b) {
    if (!*a)
      return true;
    ++a;
    ++b;
  }
  return false;
}
constexpr bool ends_with(const char *a, const char *b) {
  unsigned na = __builtin_strlen(a), nb = __builtin_strlen(b);
  return na >= nb && eq(a + (na - nb), b);
}

static_assert(ln() == 32);
static_assert(cl() == 15);
static_assert(  cl  () == 17);
static_assert(ln2() == 35);
static_assert(__builtin_LINE() == 36);
static_assert(__builtin_COLUMN() == 15);
static_assert(eq(__builtin_FILE_NAME(), "builtins-srcloc.cpp"));
static_assert(ends_with(__builtin_FILE(), "builtins-srcloc.cpp"));
static_assert(eq(__builtin_FUNCTION(), ""));
static_assert(eq(fnm(), ""));

constexpr const char *g() { return fnm(); }
static_assert(eq(g(), "g"));
constexpr const char *h() { return __builtin_FUNCTION(); }
static_assert(eq(h(), "h"));
namespace ns {
constexpr const char *m(int, const char *) { return __builtin_FUNCTION(); }
} // namespace ns
static_assert(eq(ns::m(0, nullptr), "m"));

static_assert(sl()->_M_line == 52);
static_assert(sl()->_M_column == 15);
constexpr const impl *k() { return sl(); }
static_assert(eq(k()->_M_function_name, "const impl *k()"));
static_assert(eq(k()->_M_file_name, __builtin_FILE()));
static_assert(k()->_M_line == 54);

// One object per distinct location: equal locations compare equal, the strings are
// shared with __builtin_FILE().
static_assert(__builtin_FILE() == __builtin_FILE());
static_assert(sl() != sl()); // distinct columns
static_assert(k() == k());
constexpr auto q1 = k();
constexpr auto q2 = k();
static_assert(q1 == q2);
static_assert(q1->_M_file_name == __builtin_FILE());

// The types.
static_assert(__is_same(decltype(__builtin_LINE()), unsigned));
static_assert(__is_same(decltype(__builtin_COLUMN()), unsigned));
static_assert(__is_same(decltype(__builtin_FILE()), const char *));
static_assert(__is_same(decltype(__builtin_FUNCTION()), const char *));
static_assert(__is_same(decltype(__builtin_source_location()), const impl *));

int main() { static_assert(eq(fnm(), "main")); }
