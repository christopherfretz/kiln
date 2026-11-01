// [support.srcloc.cons], [dcl.spec.auto.general], [expr.const]/16: a
// location's function name is computed when the location is evaluated, so a
// deduced return type prints once deduced (clang 19): at run time, and in an
// immediate invocation of the deducing return statement; an invocation
// evaluated earlier (an earlier statement, or a default argument built with
// the call) keeps `auto`. A consteval std::source_location::current() as a
// default argument, also of a function template, is evaluated at each call.
//@ expect: run
//@ stdout: const char *f2()
//@ stdout: auto f1()
//@ stdout: const char *f3()
//@ stdout: const char *f4()
//@ stdout: auto f6()
//@ stdout: const char *f7()
//@ stdout: const char *f9()
//@ stdout: auto f13()
//@ stdout: const char *S::m()
//@ stdout: auto h(T) [T = int]
//@ stdout: 54 int main()|55 int main()|56 int main()
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
extern "C" int printf(const char *, ...);
using I = const std::source_location::__impl *;
auto f1() { auto l = std::source_location::current(); return l.function_name(); }
auto f2() { return std::source_location::current().function_name(); }
auto f3() { const char *p = static_cast<I>(__builtin_source_location())->_M_function_name; return p; }
auto f4() { return static_cast<I>(__builtin_source_location())->_M_function_name; }
auto f5(std::source_location l = std::source_location::current()) { return l.function_name(); }
auto f6() { return f5(); }
auto f7() { if (false) return (const char *)nullptr; return std::source_location::current().function_name(); }
auto f8(const void *p = __builtin_source_location()) { return static_cast<I>(p)->_M_function_name; }
auto f9() { return f8(); }
auto f13() { constexpr auto p = __builtin_source_location(); return static_cast<I>(p)->_M_function_name; }
struct S { auto m() { return std::source_location::current().function_name(); } };
template <class T> auto h(T) { return std::source_location::current().function_name(); }
void n(std::source_location l = std::source_location::current()) { printf("%u %s|", l.line(), l.function_name()); }
template <class T> void k(T, std::source_location l = std::source_location::current()) { printf("%u %s", l.line(), l.function_name()); }
int main() {
  printf("%s\n", f2());
  printf("%s\n%s\n%s\n%s\n%s\n", f1(), f3(), f4(), f6(), f7());
  printf("%s\n%s\n", f9(), f13());
  printf("%s\n%s\n", S{}.m(), h(1));
  n();
  k(1);
  printf("|"); k('c');
  printf("\n");
}
