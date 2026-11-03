// [expr.const]/5.1, [implimits]: one step below the minimum
// std-builtins.cpp needs (a std::move, forward, forward_like,
// move_if_noexcept, as_const or addressof call costs no step).
//@ args: -stdlib=libc++ -fconstexpr-steps=605
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:3 note "constexpr evaluation hit maximum step limit; possible infinite loop?"
//@ diag: 27:15 note "in call to 'run(100)'"
#include <memory>
#include <utility>
struct P {
  int v;
};
constexpr int run(int n) {
  int sum = 0;
  P p{1};
  for (int i = 0; i < n; ++i) {
    P q = std::move(p);
    p = std::move_if_noexcept(q);
    sum += std::forward<P &>(p).v + std::as_const(p).v;
    sum += std::addressof(p)->v + std::addressof(q)->v;
    sum += std::forward_like<const P &>(p.v);
  }
  return sum;
}
static_assert(run(100) == 500);
