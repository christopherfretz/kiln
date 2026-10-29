// [support.srcloc.cons]: __builtin_source_location() needs a complete
// std::source_location::__impl with exactly the four expected fields; the
// location builtins take no arguments and must be called.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:10 error "'std::source_location::__impl' was not found; it must be defined before '__builtin_source_location' is called"
//@ diag: 16:10 error "'std::source_location::__impl' must be standard-layout and have only two 'const char *' fields '_M_file_name' and '_M_function_name', and two integral fields '_M_line' and '_M_column'"
//@ diag: 17:24 error "expected ')'"
//@ diag: 18:25 error "expected '(' after '__builtin_COLUMN'"
auto p = __builtin_source_location();
namespace std {
struct source_location {
  struct __impl { const char *_M_file_name; int x; };
};
} // namespace std
auto q = __builtin_source_location();
int a = __builtin_LINE(1);
int b = __builtin_COLUMN;
