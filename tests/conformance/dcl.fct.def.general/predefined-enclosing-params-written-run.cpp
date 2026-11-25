// [dcl.fct.def.general]/8 (`__PRETTY_FUNCTION__`, clang's
// PredefinedExpr::ComputeName): a function printed as the scope of a
// local class's member prints its parameters' types as declared (clang
// `printNestedNameSpecifier`: the parameter declarations' types), not
// the canonical prototype's: typedefs, aliases, `decltype` and function
// and array declarators keep their spelling (an array parameter prints
// adjusted, `int *`); in an instantiation the template arguments are
// substituted into the written types (`t3(int, I)`, `pk(int, char,
// ns::S)`), for a member of a class template and a member template too.
// `__FUNCTION__` and `__builtin_FUNCTION()` print no scope;
// `source_location::function_name()` prints the pretty name. The
// spelling is the same with clang 19 and 22 (no closure in a scope).
// Reduced from real code (WP-3402).
//@ expect: run
//@ std: c++20
//@ stdout: const char *g(decltype(ns::S::value))::L::m()
//@ stdout: static const char *h(I, ns::S, int *, void (*)(I))::L::m()
//@ stdout: static const char *t3(int, I)::L::m()
//@ stdout: static const char *t4(int, decltype(ns::S::value))::L::m()
//@ stdout: static const char *pk(int, char, ns::S)::L::m()
//@ stdout: static const char *ab(long, I)::L::m()
//@ stdout: static const char *dep(ns::S *, const ns::A &)::L::m()
//@ stdout: static const char *K::K(ns::A, I)::L::m()
//@ stdout: static const char *K::cm(I)::L::m()
//@ stdout: static const char *C<int>::mt(int, double, ns::A)::L::m() [T = int]
//@ stdout: static const char *fp(I (*)(ns::A), I (&)[2], const volatile I *)::L::m()
//@ stdout: static const char *nt(int (&)[2], decltype(2))::L::m()
//@ stdout: static const char *va(I, ...)::L::m()
//@ stdout: static const char *nest(I)::L::m()::M::m()
//@ stdout: m
//@ stdout: m
#include <cstdio>
#include <source_location>
namespace ns {
struct S { static constexpr int value = 1; };
using A = S;
}
using I = int;
#define LOCAL(expr) struct L { static const char *m() { return expr; } }; return L::m()
const char *g(decltype(ns::S::value)) { struct L { const char *m() { return __PRETTY_FUNCTION__; } }; return L().m(); }
const char *h(I, ns::S, int[3], void (*)(I)) { LOCAL(__PRETTY_FUNCTION__); }
template <class T> const char *t3(T, I) { LOCAL(__PRETTY_FUNCTION__); }
template <class T> const char *t4(T, decltype(ns::S::value)) { LOCAL(__PRETTY_FUNCTION__); }
template <class... Ts> const char *pk(Ts...) { LOCAL(__PRETTY_FUNCTION__); }
const char *ab(auto, I) { LOCAL(__PRETTY_FUNCTION__); }
template <class T> const char *dep(T *, const ns::A &) { LOCAL(__PRETTY_FUNCTION__); }
struct K {
  const char *s;
  K(ns::A, I = 3) { struct L { static const char *m() { return __PRETTY_FUNCTION__; } }; s = L::m(); }
  const char *cm(I) const & { LOCAL(__PRETTY_FUNCTION__); }
};
template <class T> struct C {
  template <class U> const char *mt(T, U, ns::A) { LOCAL(__PRETTY_FUNCTION__); }
};
I fi(ns::A) { return 0; }
const char *fp(I (*)(ns::A), I (&)[2], const volatile I *) {
  LOCAL(std::source_location::current().function_name());
}
template <int N> const char *nt(int (&)[N], decltype(N)) { LOCAL(__PRETTY_FUNCTION__); }
const char *va(I, ...) { LOCAL(__PRETTY_FUNCTION__); }
const char *nest(I) {
  struct L { static const char *m() { struct M { static const char *m() { return __PRETTY_FUNCTION__; } }; return M::m(); } };
  return L::m();
}
const char *fn(I) { LOCAL(__FUNCTION__); }
const char *bf(I) { LOCAL(__builtin_FUNCTION()); }
int main() {
  int arr[2] = {};
  ns::S s;
  puts(g(1));
  puts(h(1, s, arr, nullptr));
  puts(t3<I>(1, 1));
  puts(t4(1, 1));
  puts(pk(1, 'c', s));
  puts(ab(2L, 1));
  puts(dep(&s, s));
  puts(K(s).s);
  puts(K(s, 1).cm(1));
  puts(C<I>().mt(1, 2.0, s));
  puts(fp(fi, arr, nullptr));
  puts(nt(arr, 1));
  puts(va(1));
  puts(nest(1));
  puts(fn(1));
  puts(bf(1));
}
