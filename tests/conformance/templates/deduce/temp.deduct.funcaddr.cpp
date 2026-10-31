// [temp.deduct.funcaddr]: taking the address of a function template deduces P (the template's function type) from the target type; the more specialized match wins.
template <class T> constexpr T id(T x) { return x; }
constexpr int (*p)(int) = id;
constexpr long (*q)(long) = &id;
static_assert(p(3) == 3 && q(4) == 4);
template <class T> constexpr int f(T) { return 1; }
template <class T> constexpr int f(T*) { return 2; }
constexpr int (*r)(int*) = f;
constexpr int (*s)(int) = f;
static_assert(r(nullptr) == 2 && s(0) == 1);
static_assert(&id<int> == p);
