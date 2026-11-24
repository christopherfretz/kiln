// Function-name spans (spec/ast.md §24.5): a default argument's
// `__builtin_FUNCTION()` and `std::source_location::current()` are made
// at each use ([expr.const]/16), so each caller has its own string and
// record; two uses in one function share one.
//@ fn-names: per-use.names
//@ std: c++20
namespace std {
struct source_location {
  struct __impl {
    const char *_M_file_name;
    const char *_M_function_name;
    unsigned _M_line;
    unsigned _M_column;
  };
  using __bsl_ty = decltype(__builtin_source_location());
  static consteval source_location current(__bsl_ty __p = __builtin_source_location()) noexcept {
    source_location r;
    r._M_impl = static_cast<const __impl *>(__p);
    return r;
  }
  const __impl *_M_impl = nullptr;
};
}
namespace ns {
struct S {};
const char *who(const char *s = __builtin_FUNCTION()) { return s; }
void where(std::source_location l = std::source_location::current()) { (void)l; }
void a1(S) { who(); who(); where(); }
template <class T> void a2(T) { who(); where(); }
}
void use() { ns::a1({}); ns::a2(1); }
