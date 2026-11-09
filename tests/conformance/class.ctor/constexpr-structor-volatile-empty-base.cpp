// [class.ctor.general]/5, [expr.const]/10.8: an empty base at offset 0
// shares its address with the first member; its constructor running does
// not lift the volatility of the complete object or of that member. WP-1101.
//@ std: c++26
//@ expect: compile-fail
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 15:27 note "assignment to volatile object 'f' is not allowed in a constant expression"
//@ diag: 18:14 note "volatile object declared here"
#include <memory>
struct F;
struct E { constexpr E(volatile F *p); };
struct F : E { int m; constexpr F() : E(nullptr), m(1) {} };
constexpr E::E(volatile F *p) {
  if (p)
    const_cast<F *>(p)->m = 2;
}
constexpr int empty_base() {
  volatile F f;
  std::construct_at(static_cast<E *>(const_cast<F *>(&f)), &f);
  return 0;
}
static_assert(empty_base() == 0);
