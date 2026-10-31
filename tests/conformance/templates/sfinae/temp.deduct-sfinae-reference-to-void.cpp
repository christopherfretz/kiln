// [temp.deduct]/11.3: forming a reference to void fails.
template <class T> constexpr int f(T&) { return 1; }
template <class T> constexpr int f(...) { return 2; }
int i;
static_assert(f<int>(i) == 1);
static_assert(f<void>(0) == 2);
