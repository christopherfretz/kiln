// [support.srcloc.cons]/2, [temp.inst]: in a function template the
// function name a source-location value holds is the specialization's,
// with clang 19's template-argument suffix: `__builtin_FUNCTION`,
// `__builtin_source_location` and a default argument that evaluates them
// are recomputed when the template is instantiated.
//@ expect: run
//@ stdout: g|const char *g(T) [T = int]
//@ stdout: g|const char *g(T) [T = char]
//@ stdout: m|void A<long>::m(U) [T = long, U = bool]
//@ stdout: cur|const char *cur(T) [T = int]
//@ stdout: auto main()::(anonymous class)::operator()(auto) const [x:auto = int]
namespace std {
struct source_location {
  struct __impl { const char *_M_file_name; const char *_M_function_name; unsigned _M_line; unsigned _M_column; };
};
}
extern "C" int printf(const char *, ...);
struct L {
  static consteval const char *name(const char *p = __builtin_FUNCTION()) { return p; }
  static consteval const void *loc(const void *p = __builtin_source_location()) { return p; }
};
using Impl = std::source_location::__impl;
template <class T> const char *g(T) {
  printf("%s|", __builtin_FUNCTION());
  return __builtin_source_location()->_M_function_name;
}
template <class T> struct A {
  template <class U> void m(U) {
    printf("%s|%s\n", L::name(), static_cast<const Impl *>(L::loc())->_M_function_name);
  }
};
template <class T> const char *cur(T) {
  printf("%s|", L::name());
  return static_cast<const Impl *>(L::loc())->_M_function_name;
}
int main() {
  printf("%s\n", g(1));
  printf("%s\n", g('c'));
  A<long>().m(true);
  printf("%s\n", cur(1));
  auto l = [](auto x) { return __builtin_source_location()->_M_function_name; };
  printf("%s\n", l(1));
}
