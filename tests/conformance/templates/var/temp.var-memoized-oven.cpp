// [temp.inst]/7, [temp.var], [expr.const]/2 + kiln Oven budgets: each
// variable template specialization is instantiated and its constant
// initializer evaluated once, so the fib_v<N> chain costs a number of
// evaluations linear in N (not the exponential count of a recursive
// function); the budgets pin that.
//@ oven: evaluations<=300 steps<=1000 frozen_bytes<=2048
template <int N> constexpr long fib_v = fib_v<N - 1> + fib_v<N - 2>;
template <> constexpr long fib_v<0> = 0;
template <> constexpr long fib_v<1> = 1;
static_assert(fib_v<60> == 1548008755920);
static_assert(fib_v<90> == 2880067194370816120);
