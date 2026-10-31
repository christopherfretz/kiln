// [temp.deduct.call]/6: an overload set with one candidate deduces from it.
int one(int) { return 1; }
template <class F> constexpr int call(F f) { return sizeof(f); }
static_assert(call(one) == sizeof(int (*)(int)));
static_assert(call(&one) == sizeof(int (*)(int)));
