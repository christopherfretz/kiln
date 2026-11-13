// [expr.typeid]/3, [basic.def.odr]/4-5: a glvalue of polymorphic class
// type operand of `typeid` is potentially evaluated, so the functions it
// calls are odr-used and emitted: the implicitly instantiated
// `P<B>::operator*`, the conversion function `W::operator B&` and the
// inline `inl` are defined (linkonce_odr) in this module, as clang's.
// WP-1322 (found in downstream code): kiln declared `P<B>::operator*` only.
//@ expect: ir
//@ min-oracle: 22
//@ std: c++20
//@ args: -Wno-potentially-evaluated-expression
namespace std { class type_info; }
struct B { virtual ~B(); };
template <class T> struct P { T *p; T &operator*() const { return *p; } };
struct W { B *b; operator B &() const { return *b; } };
inline B &inl(B &b) { return b; }
P<B> &held();
W &w();
const std::type_info &f() { return typeid(*held()); }
const std::type_info &g() { return typeid(static_cast<B &>(w())); }
const std::type_info &h(B &b) { return typeid(inl(b)); }
