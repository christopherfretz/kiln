// [temp.deduct.call]/1: a trailing function parameter pack deduces one type per argument.
template <class T, class... Ts> constexpr T first(T t, Ts...) { return t; }
static_assert(first(5, 1.0, 'c') == 5);
static_assert(first(7) == 7);
template <class... Ts> constexpr unsigned long n(Ts...) { return sizeof...(Ts); }
static_assert(n(1, 2u, 3l, 'x') == 4);
