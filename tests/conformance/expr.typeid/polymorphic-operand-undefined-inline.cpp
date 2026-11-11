// [expr.typeid]/3, [basic.def.odr]/4,15: an inline function named in the
// operand of `typeid` is odr-used only when the operand is a glvalue of
// polymorphic class type; then it must be defined (clang's
// -Wundefined-inline). Named in a non-polymorphic operand it is not
// odr-used, and nothing is diagnosed (WP-1322).
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ args: -Wno-potentially-evaluated-expression
//@ diag: 15:11 warning "inline function 'poly' is not defined"
//@ diag: 19:43 note "used here"
namespace std { class type_info; }
struct B { virtual ~B(); };
struct NP { int x; };
inline B &poly();
inline NP &plain();
template <class T> inline T &dep();
const std::type_info &f() { return typeid(plain()); }
const std::type_info &g() { return typeid(poly()); }
const std::type_info &h() { return typeid(dep<NP>()); }
