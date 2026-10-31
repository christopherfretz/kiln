// [temp.deduct.type]/8: T* against a pointer; the pointee may be more qualified.
template <class T> constexpr T get(const T* p) { return *p; }
int x = 7;
constexpr int y = 8;
static_assert(get(&y) == 8);
int z = get(&x);
