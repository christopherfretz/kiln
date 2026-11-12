// [expr.typeid]/3, [basic.def.odr]/4: the operand of `typeid` that is a
// glvalue of polymorphic class type is potentially evaluated, so the names
// in it are odr-uses (no `non_odr_use_unevaluated`, as clang's re-transformed
// operand), except inside a nested unevaluated operand; a non-polymorphic
// operand keeps them (WP-1322).
//@ expect: ast
//@ oracle: clang
//@ std: c++20
//@ args: -Wno-potentially-evaluated-expression
namespace std { class type_info; }
struct B { virtual void v(); };
struct N { int x; };
B &pb(int);
N &pn(int);
int k;
const std::type_info &f() { return typeid(pb(noexcept(pn(k)))); }
const std::type_info &g() { return typeid(pn(k)); }
