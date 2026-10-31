// [temp.deduct.partial]/5: references are stripped; const T& against T& orders by T.
template <class T> constexpr int f(const T&) { return 1; }
template <class T> constexpr int f(T* const&) { return 2; }
int x;
int* p = &x;
static_assert(f(1) == 1);
int r = f(p);
