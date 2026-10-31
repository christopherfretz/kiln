// [temp.deduct]/11.2: naming a member of a non-class type fails.
template <class T> constexpr int f(typename T::value_type*) { return 1; }
template <class T> constexpr int f(...) { return 2; }
struct V { using value_type = int; };
static_assert(f<V>(nullptr) == 1);
static_assert(f<int>(nullptr) == 2);
