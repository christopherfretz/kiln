// [expr.const]/16, [dcl.fct.default]/9, [support.srcloc.cons]: an immediate
// invocation in a default argument (std::source_location::current()) is
// evaluated at each use, with that call's location, and lowers to its value;
// the consteval function itself is never emitted. Also in a function
// template's parameter, and in a deduced-return function whose name the
// location holds.
namespace std {
struct source_location {
  struct __impl { const char *_M_file_name; const char *_M_function_name; unsigned _M_line; unsigned _M_column; };
  static consteval source_location current(const __impl *p = static_cast<const __impl *>(__builtin_source_location())) noexcept {
    source_location l;
    l._M_impl = p;
    return l;
  }
  constexpr unsigned line() const noexcept { return _M_impl ? _M_impl->_M_line : 0u; }
  constexpr const char *function_name() const noexcept { return _M_impl ? _M_impl->_M_function_name : ""; }
  const __impl *_M_impl = nullptr;
};
}
consteval int g(int x = __builtin_LINE()) { return x; }
int f(int a = g()) { return a; }
template <class T> int t(T, int a = g() + 1) { return a; }
unsigned n(std::source_location l = std::source_location::current()) { return l.line(); }
template <class T> unsigned k(T, std::source_location l = std::source_location::current()) { return l.line(); }
auto d() { return std::source_location::current().function_name(); }
auto e() { return n(); }
int use() { return f() + t(1) + t('c') + n() + k(1) + k(2.0); }
