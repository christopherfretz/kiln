// [temp.arg.explicit]/9: explicit arguments for a pack, extended by deduction.
template <class... Ts> constexpr int sum(Ts... ts) { return (ts + ... + 0); }
static_assert(sum<int, long>(1, 2) == 3);
static_assert(sum<int>(1, 2, 3) == 6);
template <class... Ts> constexpr unsigned long n(Ts...) { return sizeof...(Ts); }
static_assert(n<int, int>(1, 2) == 2);
