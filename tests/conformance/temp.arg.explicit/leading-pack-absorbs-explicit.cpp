// [temp.arg.explicit]/9, [temp.param]/14: a function template parameter
// pack followed by a deducible parameter (`template <class... A, class T>`)
// takes every explicit template argument, however many there are, directly
// and from a pack expansion at instantiation (libc++ uses_alloc_types.h
// `checkConstruct<Args&&...>(value, form)`). Without a pack, more explicit
// arguments than parameters is a substitution failure. Matches clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:9 error "no matching function for call to 'two'"
//@ diag: 31:33 note "candidate template ignored: substitution failure: too many template arguments for function template 'two'"
template <class... E> struct tup {};

template <class... A, class T> constexpr int check(T&, int f) { return sizeof...(A) == f ? 1 : 0; }

template <class... TT> constexpr int d1(int k) { int x = 0; return check<TT&&...>(x, k); }
template <class U, class... TT, class... UT>
constexpr int d2(int k, tup<TT...>, tup<UT...>) {
  int x = 0;
  return check<TT&&...>(x, k) + check<UT...>(x, k);
}

int x;
static_assert(check<>(x, 0) == 1);
static_assert(check<int, long, char>(x, 3) == 1);
static_assert(check<int, long, char, short, int&>(x, 5) == 1);
static_assert(d1<int, long, char>(3) == 1);
static_assert(d1<int, long, char, short>(4) == 1);
static_assert(d2<int>(3, tup<int, long, char>{}, tup<int&, char, short>{}) == 2);
constexpr int (*fp)(int&, int) = &check<int, long, char, int>;

template <class A, class T> int two(T&) { return 0; }
int y = two<int>(x);
int z = two<int, int, int>(x);
