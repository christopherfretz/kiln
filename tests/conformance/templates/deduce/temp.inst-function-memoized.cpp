// [temp.inst]/9: a specialization is instantiated once; both calls name the same function.
template <class T> constexpr T id(T x) { return x; }
static_assert(&id<int> == &id<int>);
constexpr int (*p)(int) = &id<int>;
static_assert(p(3) == 3);
