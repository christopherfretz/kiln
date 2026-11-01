// [temp.dep.expr]/3, [dcl.init.list]/1, [over.ics.list]: a braced-init-list
// argument of a dependent call is kept as written and list-initializes
// its parameter at instantiation, in SFINAE contexts too. libc++
// `__is_implicitly_default_constructible`
// (`decltype(std::__test_implicit_default_constructible<_Tp const&>({}))`).
// WP-128.
//@ expect: compile-ok

template <class T> void test(T);
template <class T> struct A { using type = int; };
template <class T, class = void> struct D { static constexpr int v = 0; };
template <class T> struct D<T, decltype(test<T const&>({}))> { static constexpr int v = 1; };
struct E { explicit E() = default; };
struct I { };
static_assert(D<I>::v == 1);
static_assert(D<E>::v == 0);
static_assert(D<int>::v == 1);

struct P { int a, b; };
constexpr int sum(P p) { return p.a + p.b; }
constexpr int sum2(int x, P p) { return x + p.a * p.b; }
template <class T> constexpr int f(T t) { return sum({t, t + 1}) + sum2(t, {2, 3}); }
static_assert(f(1) == 3 + 7);
template <class T> constexpr int g(T t) { return sum2(1, {{t}, 4}); }
static_assert(g(2) == 9);

int main() {}
