// [over.match.best]/2.4: a non-template is preferred before partial ordering.
template <class T> constexpr int f(T*) { return 1; }
constexpr int f(int*) { return 2; }
int x;
long y;
static_assert(f(&x) == 2);
static_assert(f(&y) == 1);
