// [temp.deduct.type]/5.1: a nested-name-specifier is a non-deduced context.
template <class T> struct id { using type = T; };
template <class T> constexpr T add(T a, typename id<T>::type b) { return a + b; }
static_assert(add(1, 2.9) == 3);
static_assert(add(1.5, 2) == 3.5);
