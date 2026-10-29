// [expr.const]/4 (usable in constant expressions): a const-qualified
// integral variable template specialization with a constant initializer.
template <class T> const int width = sizeof(T) * 8;
template <class T> const bool is_wide = width<T> > 32;
static_assert(width<int> == 32);
static_assert(width<long> == 64);
static_assert(is_wide<long> && !is_wide<short>);
int arr[width<char>];
static_assert(sizeof(arr) == 8 * sizeof(int));
