// [temp.deduct.call]/2.1: an array argument decays for a non-reference P.
template <class T> constexpr bool is_ptr(T) { return false; }
template <class T> constexpr bool is_ptr(T*) { return true; }
template <class T> struct tag { static constexpr int v = 0; };
template <class T> struct tag<T*> { static constexpr int v = 1; };
template <class T> constexpr int kind(T) { return tag<T>::v; }
int arr[4];
static_assert(kind(arr) == 1);
static_assert(is_ptr(arr));
