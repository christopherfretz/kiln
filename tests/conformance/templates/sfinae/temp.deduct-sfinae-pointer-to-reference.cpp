// [temp.deduct]/11.4: forming a pointer to a reference fails.
template <class T> constexpr int f(T*) { return 1; }
template <class T> constexpr int f(...) { return 2; }
static_assert(f<int>(nullptr) == 1);
static_assert(f<int&>(nullptr) == 2);
