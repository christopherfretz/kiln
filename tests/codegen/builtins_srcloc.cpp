// [support.srcloc.cons], [dcl.fct.default]/9: the location builtins lower to
// constants; __builtin_source_location() is a private `.constant` object per
// location, its strings shared with __builtin_FILE(); a default argument
// takes the call site's location.
namespace std { struct source_location { struct __impl { const char *_M_file_name; const char *_M_function_name; unsigned _M_line; unsigned _M_column; }; }; }
const void *p() { return __builtin_source_location(); }
const char *f() { return __builtin_FILE(); }
const char *fn() { return __builtin_FUNCTION(); }
unsigned l() { return __builtin_LINE(); }
unsigned c() { return __builtin_COLUMN(); }
unsigned dl(unsigned x = __builtin_LINE()) { return x; }
unsigned u() { return dl(); }
const std::source_location::__impl *sl(const std::source_location::__impl *q = __builtin_source_location()) { return q; }
const std::source_location::__impl *v() { return sl(); }
