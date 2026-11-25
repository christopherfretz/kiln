// Function-name spans (spec/ast.md §24.4): a function printed as a scope
// (the enclosing function of a local class's member) prints its
// parameters' types as declared, so their names are scope entries one
// depth in naming what the declaration writes: a typedef or alias names
// the typedef or alias, a `decltype` operand's names their declarations,
// a function declarator's types theirs; an instantiation's parameters are
// the written types with the arguments substituted (`t3(ns::S, I)`: the
// argument canonical and scoped, the alias as written). WP-3402.
//@ fn-names: scope-params-written.names
//@ std: c++20
namespace ns {
struct S { static constexpr int value = 1; };
using A = S;
}
typedef int I;
void g(decltype(ns::S::value), ns::A, I (*)(ns::A)) {
  struct L { static void m() { (void)__PRETTY_FUNCTION__; } };
  L::m();
}
template <class T> void t3(T, I) {
  struct L { static void m() { (void)__PRETTY_FUNCTION__; } };
  L::m();
}
void nest(I) {
  struct L { static void m() { struct M { static void m() { (void)__PRETTY_FUNCTION__; } }; M::m(); } };
  L::m();
}
void use() { g(1, {}, nullptr); t3(ns::S{}, 1); nest(1); }
