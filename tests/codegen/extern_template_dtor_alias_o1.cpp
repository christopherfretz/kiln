//@ args: -O1 -Xclang -disable-llvm-passes
// [class.dtor]/13, [temp.explicit]/14, Itanium C++ ABI 5.1.4: at -O1 the
// discardable base-object destructor of a class whose only non-trivial
// base destructor is an `available_externally` one (an extern-template
// instantiation) is replaced by that base's D2, unless the base's is
// also `always_inline` (clang `TryEmitBaseDestructorAsAlias`).
template <class T> struct Y { T v; ~Y() { v = 0; } };
extern template struct Y<int>;
struct Z : Y<int> { ~Z() {} };
template <class T> struct W { T v; __attribute__((always_inline)) ~W() { v = 0; } };
extern template struct W<int>;
struct Q : W<int> { ~Q() {} };
void use(Z* z, Q* q) { z->~Z(); q->~Q(); }
