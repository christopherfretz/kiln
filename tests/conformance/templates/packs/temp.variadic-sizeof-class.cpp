// [expr.sizeof]/5: sizeof... of a class template's parameter pack.
template <class... Ts> struct tl { static constexpr unsigned long n = sizeof...(Ts); };
static_assert(tl<>::n == 0);
static_assert(tl<int, char, long>::n == 3);
template <int... Ns> struct il { static constexpr unsigned long n = sizeof...(Ns); };
static_assert(il<1, 2>::n == 2);
