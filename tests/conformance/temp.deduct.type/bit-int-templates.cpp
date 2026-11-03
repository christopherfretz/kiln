// [temp.deduct.type]/8, [temp.dep.type]/9: a `_BitInt(N)` with a
// value-dependent width is a dependent type; its width is substituted at
// instantiation (and checked then), and deduced from an argument of the same
// signedness as a value of type `int`; a placeholder parameter
// (`template <auto N>`) takes the bound's type ([temp.deduct.type]/20).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:29 error "signed _BitInt must have a bit size of at least 2"
//@ diag: 34:6 note "in instantiation of template class 'A<1>' requested here"
//@ diag: 35:29 error "signed _BitInt must have a bit size of at least 2"
//@ diag: 36:6 note "in instantiation of template class 'B<2>' requested here"
//@ diag: 37:12 error "no matching function for call to 'g'"
//@ diag: 20:37 note "candidate template ignored: could not match 'unsigned _BitInt(N)' against '_BitInt(13)'"
template <int N> struct A { _BitInt(N) v; using T = unsigned _BitInt(N); };
A<8> a8;
static_assert(sizeof(A<8>::T) == 1 && sizeof(A<16>::T) == 2);
template <class T> using BI = _BitInt(sizeof(T) * 8);
static_assert(__is_same(BI<short>, _BitInt(16)));
template <int N> constexpr int f(_BitInt(N)) { return N; }
template <unsigned N> constexpr int g(unsigned _BitInt(N)) { return N; }
constexpr _BitInt(13) x = 0;
constexpr unsigned _BitInt(13) ux = 0;
static_assert(f(x) == 13 && g(ux) == 13);
static_assert(f(_BitInt(9)(0)) == 9);
template <class T> struct C;
template <int N> struct C<_BitInt(N)> { static constexpr int w = N; };
template <int N> struct C<unsigned _BitInt(N)> { static constexpr int w = -N; };
static_assert(C<_BitInt(21)>::w == 21 && C<unsigned _BitInt(5)>::w == -5);
template <auto N> constexpr auto h(_BitInt(N)) { return N; }
static_assert(__is_same(decltype(h(x)), int));
template <auto N> constexpr auto h2(const int (&)[N]) { return N; }
constexpr int arr[3] = {};
static_assert(__is_same(decltype(h2(arr)), decltype(sizeof 0)) && h2(arr) == 3);
A<1> bad;
template <int N> struct B { _BitInt(N - 1) v; };
B<2> bad2;
int bad3 = g(x);
