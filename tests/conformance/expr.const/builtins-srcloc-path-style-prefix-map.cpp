// [support.srcloc.cons], [cpp.predefined]: -fmacro-prefix-map= applies
// after the default (clang, as written) spelling (spec/driver.md): an
// entry for the as-written `<dir>//Inputs/../Inputs/./` of a header found
// through that `-I` replaces it, for the builtins and __FILE__ alike, as
// clang 19 and 22 do.
//@ args: -fmacro-prefix-map=%{test_dir}//Inputs/../Inputs/./=/inc/ -I %{test_dir}//Inputs/../Inputs/./
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
static_assert(eq(h_macro, "/inc/srcloc-path-style.h"));
