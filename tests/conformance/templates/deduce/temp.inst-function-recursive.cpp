// [temp.inst]: a function template specialization calling another specialization.
template <class T> constexpr T fact(T n) { return n <= 1 ? T(1) : n * fact(n - 1); }
static_assert(fact(5) == 120);
static_assert(fact(10L) == 3628800L);
