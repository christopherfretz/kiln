// [expr.prim.fold]: folds over a non-type template parameter pack.
template <int... Ns> constexpr int sum = (Ns + ... + 0);
template <int... Ns> constexpr int mul = (1 * ... * Ns);
static_assert(sum<1, 2, 3> == 6);
static_assert(sum<> == 0);
static_assert(mul<2, 3, 4> == 24);
