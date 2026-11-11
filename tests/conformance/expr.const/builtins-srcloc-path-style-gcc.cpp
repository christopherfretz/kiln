// [support.srcloc.cons]: under -fkiln-source-path-style=gcc (spec/driver.md)
// __builtin_FILE(), __builtin_FILE_NAME() and source_location::current()
// (also as a default argument called from the header, [dcl.fct.default]/9)
// spell a header found through the system directory
// `-isystem <dir>//Inputs/../Inputs/./` as GCC 16.1 does: its realpath, which
// is shorter (-fcanonical-system-headers, GCC's default; a header found
// through -I keeps the -I spelling, meta.reflection.names/
// source-location-of-include-path.cpp). __FILE__ keeps clang's spelling, and
// the main file is kept as written.
//@ args: -fkiln-source-path-style=gcc -isystem %{test_dir}//Inputs/../Inputs/./
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
constexpr bool contains(const char *s, const char *part) {
  unsigned ns = __builtin_strlen(s), np = __builtin_strlen(part);
  for (unsigned i = 0; i + np <= ns; ++i)
    if (__builtin_memcmp(s + i, part, np) == 0)
      return true;
  return false;
}
static_assert(h_file[0] == '/' && h_file[1] != '/');
static_assert(!contains(h_file, "//"));
static_assert(!contains(h_file, "/./"));
static_assert(!contains(h_file, "/../"));
static_assert(ends_with(h_file, "/expr.const/Inputs/srcloc-path-style.h"));
static_assert(eq(h_file_name, "srcloc-path-style.h"));
static_assert(eq(h_current, h_file));
static_assert(eq(h_where, h_file));
static_assert(ends_with(h_macro, "/expr.const//Inputs/../Inputs/./srcloc-path-style.h"));
static_assert(ends_with(where(), "builtins-srcloc-path-style-gcc.cpp"));
static_assert(ends_with(__builtin_FILE(), "builtins-srcloc-path-style-gcc.cpp"));
