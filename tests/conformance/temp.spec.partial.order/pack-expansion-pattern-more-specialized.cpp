// [temp.spec.partial.order], [temp.deduct.partial]/8, [temp.deduct.type]/9:
// a trailing pack expansion of `P` deduces from a pack expansion of `A` by
// matching pattern against pattern (`T*...` from `T'*...`, `box<T>...`,
// `const T...`, two uses of the pack), for class and variable template
// partial specializations and function templates alike. WP-958.
//@ std: c++20
//@ expect: compile-ok
template <typename... Types> struct tuple;
template <class T> struct box;
template <class T, class U> struct pair;
// Class partial ordering through a class-template pattern and a const pattern.
template <class> struct Y { static const int v = 0; };
template <class H, class... T> struct Y<tuple<H, box<T>...>> { static const int v = 1; };
template <class H, class... T> struct Y<tuple<box<H>, box<T>...>> { static const int v = 2; };
static_assert(Y<tuple<box<int>, box<char>>>::v == 2);
static_assert(Y<tuple<int, box<char>>>::v == 1);
template <class> struct Z { static const int v = 0; };
template <class H, class... T> struct Z<tuple<H, const T...>> { static const int v = 1; };
template <class H, class... T> struct Z<tuple<const H, const T...>> { static const int v = 2; };
static_assert(Z<tuple<const int, const char>>::v == 2);
// Two packs in one pattern.
template <class> struct W { static const int v = 0; };
template <class H, class... T> struct W<tuple<H, pair<T, T>*...>> { static const int v = 1; };
template <class H, class... T> struct W<tuple<H*, pair<T, T>*...>> { static const int v = 2; };
static_assert(W<tuple<int*, pair<int, int>*>>::v == 2);
// Function templates.
template <class H, class... T> int f(tuple<H, T*...> *) { return 1; }
template <class H, class... T> int f(tuple<H*, T*...> *) { return 2; }
int g(tuple<int*, int*> *p) { return f(p); }
// Variable templates.
template <class> constexpr int vt = 0;
template <class H, class... T> constexpr int vt<tuple<H, T&...>> = 1;
template <class H, class... T> constexpr int vt<tuple<H&, T&...>> = 2;
static_assert(vt<tuple<int&, int&>> == 2);
