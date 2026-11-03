// [expr.const]/5.1, [implimits], [forward], [utility.as.const],
// [specialized.addressof]: clang evaluates a constexpr std::move, forward,
// forward_like, move_if_noexcept, as_const and addressof without a call
// (its `isStdBuiltin`), so they cost no step: this loop needs exactly 606
// steps on kiln and clang. spec/oven.md "Limits and metrics".
//@ args: -stdlib=libc++ -fconstexpr-steps=606
//@ expect: compile-ok
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
