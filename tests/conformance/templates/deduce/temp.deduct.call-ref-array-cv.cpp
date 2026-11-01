// [temp.deduct.call]/3, [basic.type.qualifier]/3: a const array is a
// const type, so `const T&` from `const char[3]` deduces `T = char[3]`
// (the element's cv cancels against P's), and `T&` keeps it.
//@ expect: compile-ok
//@ strict-diags
template<class T> constexpr bool f(const T&) { return __is_same(T, char[3]); }
static_assert(f("ab"));
template<class T> constexpr bool g(const volatile T&) { return __is_same(T, char[3]); }
static_assert(g("ab"));
const int ci[2] = {1, 2};
template<class T> constexpr bool h(T&) { return __is_same(T, const int[2]); }
static_assert(h(ci));
template<class T> constexpr bool k(const T (&)[2]) { return __is_same(T, int); }
static_assert(k(ci));
volatile const int vci[1] = {1};
template<class T> constexpr bool m(const T&) { return __is_same(T, volatile int[1]); }
static_assert(m(vci));
