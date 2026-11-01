// [temp.deduct.call]/4.1: the type a reference parameter refers to may be
// more cv-qualified than the argument. For an array, the qualifiers are the
// element's: `const T (&)[N]` binds `int[3]` with `T = int` (std::size).
//@ expect: compile-ok
typedef unsigned long size_t;
template <class T, size_t N> constexpr size_t size(const T (&)[N]) noexcept { return N; }
template <class T> constexpr size_t extent(const volatile T (&)[]) { return 0; }
template <class T, size_t N> constexpr const T *first(const T (&a)[N]) { return a; }
template <class T> struct is_int { static constexpr bool value = false; };
template <> struct is_int<int> { static constexpr bool value = true; };
template <class T, size_t N> constexpr bool elem_int(const T (&)[N]) { return is_int<T>::value; }
template <class T> constexpr bool f() {
  T val[2] = {};
  return size(val) == 2;
}
int a[3];
extern int u[];
const int c[4] = {};
static_assert(size(a) == 3);
static_assert(size(c) == 4);
static_assert(extent(u) == 0);
static_assert(elem_int(a) && elem_int(c));
static_assert(f<int>() && f<long>());
static_assert(first(c) == c);
int main() {}
