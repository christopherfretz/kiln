// [over.match.viable]: default arguments and ellipses make candidates
// viable; the best of the viable ones is chosen.
//@ expect: compile-ok
template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
char f(int);
int f(long, int = 0, int = 0);
long g(...);
char g(int, ...);
int g(int, int);
char h(long, int = 0);
int h(double);
static_assert(same<decltype(f(1)), char>::value);
static_assert(same<decltype(f(1, 2)), int>::value);
static_assert(same<decltype(f(1, 2, 3)), int>::value);
static_assert(same<decltype(g()), long>::value);
static_assert(same<decltype(g(1)), char>::value);
static_assert(same<decltype(g(1, 2)), int>::value);
static_assert(same<decltype(g(1, 2, 3)), char>::value);
static_assert(same<decltype(g(1.5, 2)), int>::value);
static_assert(same<decltype(h(1L)), char>::value);
static_assert(same<decltype(h(1.0)), int>::value);
