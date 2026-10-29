// [temp.expl.spec]/1, [temp.inst]/8: a recursive variable template ends at
// its explicit specializations; each specialization is instantiated once.
template <unsigned N> constexpr unsigned long fib = fib<N - 1> + fib<N - 2>;
template <> constexpr unsigned long fib<0> = 0;
template <> constexpr unsigned long fib<1> = 1;
static_assert(fib<10> == 55);
static_assert(fib<50> == 12586269025ul);
template <unsigned N> constexpr unsigned long fact = N * fact<N - 1>;
template <> constexpr unsigned long fact<0> = 1;
static_assert(fact<10> == 3628800);
