// [temp.inst]/8, [temp.point]/1: a constexpr function template
// specialization is instantiated at its use, so mutually recursive
// constexpr templates (one forward-declared) reach a specialization whose
// instantiation is already in progress; that use does not instantiate it
// again (libc++ 22 `__stable_sort` <-> `__stable_sort_move`, both
// `_LIBCPP_CONSTEXPR_SINCE_CXX26`, called with explicit template
// arguments).
//@ expect: run
//@ strict-diags

template <class T> constexpr T odd(T n);
template <class T> constexpr T even(T n) { return n == 0 ? 1 : odd(n - 1); }
template <class T> constexpr T odd(T n) { return n == 0 ? 0 : even(n - 1); }
static_assert(even(10) == 1 && odd(7) == 1 && even(7) == 0);
template <class P, class C, class I> constexpr int f(I i, C c);
template <class P, class C, class I> constexpr int g(I i, C c) { return i ? f<P, C>(i - 1, c) : 7; }
template <class P, class C, class I> constexpr int f(I i, C c) { return i ? g<P, C>(i - 1, c) : 3; }
struct L {};
constexpr L l{};
static_assert(f<int, const L&>(4, l) == 3 && f<int, const L&>(3, l) == 7);
template <class T> constexpr void a(T x, long n);
template <class T> constexpr void b(T x, long n) { if (n) a(x, n); }
template <class T> constexpr void a(T x, long n) { if (n) b(x, n - 1); }
int main() {
  a(1, 3);
  return even(4) - 1 + f<int, const L&>(2, l) - 3;
}
