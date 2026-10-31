// [temp.inst]: a lambda in a function or class template is instantiated
// with its enclosing template; each specialization has its own closure.
template <class T> constexpr T twice(T v) { return [](T x) { return x + x; }(v); }
static_assert(twice(21) == 42);
static_assert(twice(1.5) == 3.0);
template <class T> struct Holder {
  static constexpr T get(T v) { return [](T x) -> T { return x * 3; }(v); }
};
static_assert(Holder<int>::get(2) == 6);
static_assert(Holder<long>::get(3) == 9L);
template <class T> constexpr T (*fp)(T) = [](T x) { return x - 1; };
static_assert(fp<int>(1) == 0);
