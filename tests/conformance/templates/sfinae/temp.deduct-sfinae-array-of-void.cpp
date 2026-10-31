// [temp.deduct]/11.1: forming an array of void or of size zero fails.
template <class T> constexpr int f(T (*)[1]) { return 1; }
template <class T> constexpr int f(...) { return 2; }
static_assert(f<int>(nullptr) == 1);
static_assert(f<void>(nullptr) == 2);
