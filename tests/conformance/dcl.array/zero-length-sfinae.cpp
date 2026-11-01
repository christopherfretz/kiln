// [temp.deduct]/11 (clang BuildArrayType, WP-166): a zero-length array is a
// GNU extension accepted outside SFINAE, but forming one during deduction or
// partial-specialization matching is a substitution failure, so `int[0]`
// matches neither `T[N]` nor `T[]` (libc++ is_bounded_array).
// Outside SFINAE (a variable, a member, an alias) it is accepted.
template <class> struct B { static constexpr bool v = false; };
template <class T, unsigned long N> struct B<T[N]> { static constexpr bool v = true; };
static_assert(!B<int[0]>::v);
static_assert(B<int[1]>::v);
template <class T, unsigned long N> constexpr int f(T(&)[N]) { return 1; }
constexpr int f(...) { return 0; }
int a0[0];
static_assert(f(a0) == 0);
template <class T> struct C { static constexpr bool v = false; };
template <class T> struct C<T[]> { static constexpr bool v = true; };
static_assert(!C<int[0]>::v);
template <int N> struct S { int a[N]; };
S<0> s;
static_assert(sizeof(S<0>) == 0);
template <int N> constexpr int g(int (*)[N]) { return 1; }
template <int N> constexpr int g(...) { return 0; }
static_assert(g<0>(nullptr) == 0);
static_assert(g<2>(nullptr) == 1);
template <int N> using A = int[N];
A<0> z;
template <class T, int N = sizeof(T[0])> constexpr bool h(int) { return true; }
template <class T> constexpr bool h(...) { return false; }
static_assert(!h<int>(0));
