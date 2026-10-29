// [temp.spec.partial.match]/1.3, [temp.spec.partial.order]: two matching
// partial specializations, neither more specialized, are ambiguous.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "ambiguous partial specializations of 'A<int, int>'"
//@ diag: 12:27 note "partial specialization matches [with T = int]"
//@ diag: 13:27 note "partial specialization matches [with T = int]"
//@ diag: 20:15 error "ambiguous partial specializations of 'w<int, int>'"
//@ diag: 17:34 note "partial specialization matches [with T = int]"
//@ diag: 18:34 note "partial specialization matches [with T = int]"
template <class T, class U> struct A { static constexpr int v = 0; };
template <class T> struct A<T, int> { static constexpr int v = 1; };
template <class T> struct A<int, T> { static constexpr int v = 2; };
static_assert(A<char, int>::v == 1);
static_assert(A<int, int>::v == 1);
template <class T, class U> constexpr int w = 0;
template <class T> constexpr int w<T, int> = 1;
template <class T> constexpr int w<int, T> = 2;
static_assert(w<int, char> == 2);
static_assert(w<int, int> == 1);
