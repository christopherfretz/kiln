// [expr.prim.lambda.closure]/7, [temp.names]/4: the call operator template
// of a generic lambda with a template-parameter-list is named with
// `x.template operator()<T>()` (`template` optional outside a dependent
// context), and an auto NTTP whose default argument is a closure is
// called through the template parameter.
//@ expect: run
//@ stdout: 4 1 8 5 9 11
#include <cstdio>
constexpr auto g = []<class T>() { return sizeof(T); };
static_assert(g.template operator()<int>() == sizeof(int));
static_assert(g.operator()<char>() == 1);
template <class L> constexpr int callg(L l) { return (int)l.template operator()<double>(); }
static_assert(callg(g) == sizeof(double));
template <auto F = [](int x) { return x + 1; }> constexpr int call(int v) { return F(v); }
static_assert(call(4) == 5);
template <auto F> struct Holder { static constexpr int v = F(); };
static_assert(Holder<[] { return 9; }>::v == 9);
inline constexpr auto lam = [] { return 11; };
int main() {
  std::printf("%d %d %d %d %d %d\n", (int)g.template operator()<int>(), (int)g.operator()<char>(),
              callg(g), call(4), Holder<[] { return 9; }>::v, Holder<lam>::v);
}
