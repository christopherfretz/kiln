// [temp.func.order]: ordering applies with explicit template arguments.
template <class T> constexpr int f(T) { return 1; }
template <class T> constexpr int f(T*) { return 2; }
static_assert(f<int>(0) == 1);
static_assert(f<int>(nullptr) == 2);
