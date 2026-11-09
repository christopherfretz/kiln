// [temp.arg.explicit]/9, [temp.deduct.type]/9: deduction extends an
// explicitly specified pack wherever a parameter type deduces it: a
// template-id, two parameters, a nested template-id, a pointer pattern,
// beside a deduced function parameter pack, in constant evaluation. WP-958.
//@ std: c++20
//@ expect: compile-ok
template <class... T> struct tuple {};
template <class T, class U> struct pair {};
template <class... T> int a(tuple<T...>) { return sizeof...(T); }
template <class... T> int b(tuple<T...>, tuple<T...>) { return sizeof...(T); }
template <class... T> int c(pair<tuple<T...>, int>) { return sizeof...(T); }
template <class... T> int d(tuple<T*...>) { return sizeof...(T); }
template <class... T, class... U> int e(tuple<T...>, U...) { return sizeof...(T) * 10 + sizeof...(U); }
static_assert(sizeof(a<int>(tuple<int, char, long>())) == sizeof(int));
void test() {
  tuple<int, char, long> t;
  int x = a<int>(t) + a<int, char>(t) + b<int>(t, t) + c<int>(pair<tuple<int, char>, int>());
  x += d<int>(tuple<int*, char*>()) + e<int>(t, 1, 2);
  (void)x;
}
template <class... T> constexpr int f(tuple<T...>) { return sizeof...(T); }
static_assert(f<int>(tuple<int, char>()) == 2);
static_assert(f<int, char>(tuple<int, char>()) == 2);
