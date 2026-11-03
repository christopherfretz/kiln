// [temp.inst]/2: printing a class template specialization's name drops a
// trailing argument equal to its parameter's default only when the default
// has a shape clang's `isSubstitutedType` matches structurally; a default
// such as `typename T::value_type` or `decltype(T::f())` is kept and never
// substituted, so naming `M<S<int>, int>` does not instantiate `S<int>`
// (libc++ `memory_alignment<simd<bool>, bool>`).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:25 error "no member named 'nope' in 'M<S<int>, int>'"
//@ diag: 25:24 error "no member named 'nope' in 'P<int>'"
//@ diag: 26:26 error "no member named 'nope' in 'D<S<char>, int>'"
//@ diag: 27:22 error "no member named 'nope' in 'M<int, int>'"
template <class T> struct S {
  static_assert(sizeof(T) == 0, "instantiated");
  using value_type = T;
};
template <class T, class U = typename T::value_type> struct M {};
template <class T, class U = T *> struct P {};
template <class T, class U = decltype(T::f())> struct D {};
template <class T> using Al = M<T, int>;

using X = M<S<int>, int>;
using Y = Al<S<long>>;
int a = M<S<int>, int>::nope;
int b = P<int, int *>::nope;
int c = D<S<char>, int>::nope;
int d = M<int, int>::nope;
