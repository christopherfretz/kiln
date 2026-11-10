// [support.srcloc.cons], [cpp.predefined]: -fmacro-prefix-map= applies
// after the -fkiln-source-path-style=gcc spelling (spec/driver.md): the
// canonical `<dir>/Inputs/` prefix of a header found through the system
// directory `-isystem <dir>//Inputs/../Inputs/./` is replaced; __FILE__,
// spelled as written, does not start with it.
//@ args: -fkiln-source-path-style=gcc -fmacro-prefix-map=%{test_dir}/Inputs/=/inc/ -isystem %{test_dir}//Inputs/../Inputs/./
//@ skip(clang): -fkiln-source-path-style= is a kiln option
//@ expect: compile-ok
//@ strict-diags
namespace std {
struct source_location {
  struct __impl { const char *_M_file_name; const char *_M_function_name; unsigned _M_line; unsigned _M_column; };
  static consteval source_location current(const __impl *p = __builtin_source_location()) {
    source_location s;
    s.p_ = p;
    return s;
  }
  constexpr const char *file_name() const { return p_->_M_file_name; }
  const __impl *p_ = nullptr;
};
} // namespace std
constexpr const char *where(std::source_location l = std::source_location::current()) {
  return l.file_name();
}
#include <srcloc-path-style.h>
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
static_assert(eq(h_file, "/inc/srcloc-path-style.h"));
static_assert(eq(h_file_name, "srcloc-path-style.h"));
static_assert(eq(h_current, "/inc/srcloc-path-style.h"));
static_assert(eq(h_where, "/inc/srcloc-path-style.h"));
static_assert(ends_with(h_macro, "/expr.const//Inputs/../Inputs/./srcloc-path-style.h"));
