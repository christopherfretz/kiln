// [temp.deduct.call]: nullptr deduces std::nullptr_t, not a pointer.
template <class T> constexpr int k(T) { return 0; }
template <class T> constexpr int k(T*) { return 1; }
static_assert(k(nullptr) == 0);
static_assert(k((int*)nullptr) == 1);
