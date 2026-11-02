// [over.match.class.deduct]/1.8, /3 (P1814, P1816): the aggregate deduction
// candidate of the class template an alias names, transformed like any
// other guide: braced, parenthesized ([dcl.init.general]/16.6.2.2) and
// designated initializers, an omitted trailing member, an alias that fixes
// or repeats the class's parameters, or deduces an array bound.
//@ expect: run
template <class T, class U> struct Ag { T x; U y; };
template <class T> using AgI = Ag<T, int>;
constexpr AgI a1{1.0, 2};
static_assert(__is_same(decltype(a1), const Ag<double, int>));
constexpr AgI a2(1.5f, 2);
static_assert(__is_same(decltype(a2), const Ag<float, int>));
constexpr AgI a3{.x = 'c', .y = 3};
static_assert(__is_same(decltype(a3), const Ag<char, int>));
constexpr AgI a4 = {2L};
static_assert(__is_same(decltype(a4), const Ag<long, int>) && a4.y == 0);

template <class T> using AgS = Ag<T, T>;
constexpr AgS s1{1, 2};
static_assert(__is_same(decltype(s1), const Ag<int, int>));

template <class T, unsigned N> struct Ar { T e[N]; };
template <unsigned N> using ArI = Ar<int, N>;
constexpr ArI r1{{1, 2, 3}};
static_assert(__is_same(decltype(r1), const Ar<int, 3>));

template <class T> struct W { T v; int k = 7; };
template <class T> using WA = W<T>;
constexpr WA w{5u};
static_assert(__is_same(decltype(w), const W<unsigned>) && w.k == 7);

int main() { return a1.y + a3.y + s1.x + r1.e[0] + int(w.v) - 12; }
