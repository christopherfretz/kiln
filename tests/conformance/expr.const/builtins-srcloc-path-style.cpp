// [support.srcloc.cons], [cpp.predefined]: the file of __builtin_FILE(),
// __builtin_FILE_NAME() and source_location::current() in a header found
// through `-I <dir>//Inputs/../Inputs/./` is clang's presumed name as
// written (the default -fkiln-source-path-style=clang; spec/driver.md),
// as __FILE__; a default argument current() takes the header's call site
// ([dcl.fct.default]/9). Matches clang 19 and 22 byte for byte.
//@ args: -I %{test_dir}//Inputs/../Inputs/./
//@ expect: run
//@ strict-diags
//@ stdout: /expr.const//Inputs/../Inputs/./srcloc-path-style.h
//@ stdout: srcloc-path-style.h
//@ stdout: /expr.const//Inputs/../Inputs/./srcloc-path-style.h
//@ stdout: /expr.const//Inputs/../Inputs/./srcloc-path-style.h
//@ stdout: /expr.const//Inputs/../Inputs/./srcloc-path-style.h
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
extern "C" int printf(const char *, ...);
extern "C" char *strstr(const char *, const char *);

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
constexpr const char *tail = "/expr.const//Inputs/../Inputs/./srcloc-path-style.h";
static_assert(h_file[0] == '/');
static_assert(ends_with(h_file, tail));
static_assert(eq(h_file_name, "srcloc-path-style.h"));
static_assert(eq(h_current, h_file));
static_assert(eq(h_where, h_file));
static_assert(eq(h_macro, h_file));
static_assert(ends_with(where(), "builtins-srcloc-path-style.cpp"));

int main() {
  const char *files[] = {h_file, h_file_name, h_current, h_where, h_macro};
  for (const char *f : files) {
    const char *t = strstr(f, "/expr.const/");
    printf("%s\n", t ? t : f);
  }
}
