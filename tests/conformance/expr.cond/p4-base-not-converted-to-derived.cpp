// [expr.cond]/4.3: when the operands' classes are the same or one is a base
// of the other, only the derived (or same) class operand is converted to
// match the base; a base class operand is never converted to the derived
// class, even through a converting constructor `D(B)`. So `c ? B() : D()`
// is a `B` (the `D` is sliced), not ambiguous. clang and GCC agree.
//@ expect: run
//@ std: c++20
//@ strict-diags
struct B { int val; };
struct D : B {
  constexpr D(int i) : B{i + 100} {}
  constexpr D(B b) : B{b.val + 1000} {}
};
template <class T> T&& dv();
static_assert(__is_same(decltype(false ? dv<B>() : dv<D>()), B&&));
static_assert(__is_same(decltype(false ? dv<D&>() : dv<B&>()), B&));
static_assert(__is_same(decltype(false ? B{1} : D{2}), B));
static_assert(__is_same(decltype(false ? dv<const B&>() : dv<const D&>()), const B&));
constexpr int pick(bool c) { return (c ? B{1} : D{2}).val; }
static_assert(pick(true) == 1 && pick(false) == 102);
int main(int argc, char**) {
  B b{7};
  D d{8};
  int r = (argc > 5 ? b : d).val;
  return r == 108 ? 0 : 1;
}
