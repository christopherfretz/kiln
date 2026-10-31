// [temp.arg.explicit]/4: a trailing pack not deduced from anything is empty.
template <class... Ts> constexpr unsigned long n() { return sizeof...(Ts); }
static_assert(n() == 0);
static_assert(n<int, char>() == 2);
template <class T, class... Ts> constexpr unsigned long m() { return sizeof...(Ts); }
static_assert(m<int>() == 0);
