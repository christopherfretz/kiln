// [expr.prim.lambda.closure], [dcl.fct.def.general]/8 (`__PRETTY_FUNCTION__`,
// clang's PredefinedExpr::ComputeName): the function enclosing a closure
// (or a local class in one) prints its parameters' types as declared,
// not the canonical prototype's: `g(decltype(ns::S::value))`, nested
// closures and a local class in a closure, a function template's
// instantiation with its arguments substituted into the written types
// (`t2(int, decltype(ns::S::value))`), a member of a class template,
// `source_location::function_name()` alike. A substituted `T::` prints
// its replacement fully scoped (clang 22: `typename W<S>::type`,
// `(anonymous namespace)::An::`) with clang 22; this is the clang 19 form
// (`-fclang-compat=19`): the closure is `(anonymous class)` and the
// qualifier prints with its scope suppressed (`typename W<S>::type`).
// Reduced from real code (WP-3402).
//@ expect: run
//@ std: c++20
//@ requires: clang<22
//@ stdout: auto g(decltype(ns::S::value))::(anonymous class)::operator()() const
//@ stdout: auto n1(I)::(anonymous class)::operator()()::(anonymous class)::operator()() const
//@ stdout: const char *lc(I)::(anonymous class)::operator()()::M::m()
//@ stdout: auto t2(int, decltype(ns::S::value))::(anonymous class)::operator()() const
//@ stdout: auto C<int>::f(int, I)::(anonymous class)::operator()() const [T = int]
//@ stdout: auto sl(I)::(anonymous class)::operator()() const
//@ stdout: auto va(I, ...)::(anonymous class)::operator()() const
//@ stdout: static const char *gl(ns::S)::(anonymous class)::operator()(int)::L::m()
//@ stdout: static const char *dep(typename S::type)::L::m()
//@ stdout: static const char *dep(typename In::type)::L::m()
//@ stdout: static const char *dep(typename W<S>::type)::L::m()
//@ stdout: static const char *dep(typename An::type)::L::m()
//@ stdout: static const char *dep(typename G::type)::L::m()
//@ stdout: auto W<ns::S>::f(typename S::type)::(anonymous class)::operator()() const [T = ns::S]
//@ stdout: operator()
#include <cstdio>
#include <source_location>
namespace ns {
struct S { static constexpr int value = 1; using type = long; struct In { using type = int; }; };
template <class U> struct W { using type = U; };
}
namespace { struct An { using type = int; }; }
struct G { using type = short; };
using I = int;
const char *g(decltype(ns::S::value)) { return [] { return __PRETTY_FUNCTION__; }(); }
const char *n1(I) { return [] { return [] { return __PRETTY_FUNCTION__; }(); }(); }
const char *lc(I) { return [] { struct M { const char *m() { return __PRETTY_FUNCTION__; } }; return M().m(); }(); }
template <class T> const char *t2(T, decltype(ns::S::value)) { return [] { return __PRETTY_FUNCTION__; }(); }
template <class T> struct C { const char *f(T, I) { return [] { return __PRETTY_FUNCTION__; }(); } };
const char *sl(I) { return [] { return std::source_location::current().function_name(); }(); }
const char *va(I, ...) { return [] { return __PRETTY_FUNCTION__; }(); }
template <class T> const char *gl(T) {
  return [](auto) { struct L { static const char *m() { return __PRETTY_FUNCTION__; } }; return L::m(); }(0);
}
template <class T> const char *dep(typename T::type) {
  struct L { static const char *m() { return __PRETTY_FUNCTION__; } };
  return L::m();
}
template <class T> struct W { const char *f(typename T::type) { return [] { return __PRETTY_FUNCTION__; }(); } };
const char *fn(I) { return [] { return __func__; }(); }
int main() {
  ns::S s;
  puts(g(1));
  puts(n1(1));
  puts(lc(1));
  puts(t2(1, 1));
  puts(C<I>().f(1, 1));
  puts(sl(1));
  puts(va(1));
  puts(gl(s));
  puts(dep<ns::S>(1));
  puts(dep<ns::S::In>(1));
  puts(dep<ns::W<ns::S>>({}));
  puts(dep<An>(1));
  puts(dep<G>(1));
  puts(W<ns::S>().f(1));
  puts(fn(1));
}
