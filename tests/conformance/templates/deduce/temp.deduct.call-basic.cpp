// [temp.deduct.call]/1: T deduced from the argument type.
template <class T> constexpr T id(T x) { return x; }
static_assert(id(3) == 3);
static_assert(id('a') == 'a');
static_assert(sizeof(id(3L)) == sizeof(long));
static_assert(id(2.5) == 2.5);
