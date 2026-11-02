//@ requires: clang>=22
//@ strict-diags
// [dcl.struct.bind]/1 (C++26 P1061): a structured binding pack introduces
// one binding per remaining element, for arrays (/3), tuple-like types
// (/4, `get<i>` and `std::tuple_element`) and data members (/5), and is
// usable wherever a pack is ([temp.variadic]/5): `sizeof...`, fold
// expressions, call arguments, pack indexing ([expr.prim.pack.index]),
// lambda captures and nested expansions. An empty pack is allowed.
namespace std {
  template <class T> struct tuple_size;
  template <unsigned long I, class T> struct tuple_element;
}
struct P { int x; long y; char z; };
struct T3 {
  int a; double b; int c;
  template <unsigned long I> constexpr const auto& get() const {
    if constexpr (I == 0) return a; else if constexpr (I == 1) return b; else return c;
  }
};
template <> struct std::tuple_size<T3> { static constexpr unsigned long value = 3; };
template <> struct std::tuple_element<0, T3> { using type = const int; };
template <> struct std::tuple_element<1, T3> { using type = const double; };
template <> struct std::tuple_element<2, T3> { using type = const int; };

constexpr long sum3(int a, long b, char c) { return a + b + c; }
template <class U> constexpr int count(const U& u) { auto [...xs] = u; return sizeof...(xs); }
template <class U> constexpr long total(const U& u) { auto [a, ...xs] = u; return (a + ... + xs); }
template <class U> constexpr int last(const U& u) { auto& [...xs] = u; return xs...[sizeof...(xs) - 1]; }
template <class U> constexpr long call(const U& u) { auto& [...xs] = u; return sum3(xs...); }
template <class U> constexpr int mid(const U& u) {
  auto [a, ...xs, b] = u;
  return sizeof...(xs) * 100 + a * 10 + b;
}
template <class U> constexpr int empty(const U& u) { auto [a, b, c, ...xs] = u; return sizeof...(xs); }

constexpr int arr[4] = {1, 2, 3, 4};
static_assert(count(P{1, 2, 3}) == 3);
static_assert(total(P{1, 2, 3}) == 6);
static_assert(call(P{1, 2, 3}) == 6);
static_assert(count(arr) == 4);
static_assert(last(arr) == 4);
static_assert(mid(arr) == 214);
static_assert(empty(P{}) == 0);
static_assert(count(T3{1, 2.0, 3}) == 3);
static_assert(total(T3{1, 2.0, 3}) == 6);

// A generic lambda is a templated entity, even in a non-template function.
constexpr int lam() {
  auto l = [](auto q) { auto [...zs] = q; return (zs * ... * 1); };
  return l(P{2, 3, 4});
}
static_assert(lam() == 24);

// Range-for and condition declarations.
template <class R> constexpr int loop(const R& r) {
  int s = 0;
  for (auto [a, ...rest] : r) s += a * 10 + sizeof...(rest);
  return s;
}
constexpr P ps[2] = {{1, 2, 3}, {4, 5, 6}};
static_assert(loop(ps) == 54);
template <class T> constexpr bool cond(T t) {
  if (auto [...xs] = t; sizeof...(xs) == 3) return true;
  return false;
}
static_assert(cond(P{}));

// Captured by reference and by copy, and expanded with another pack.
template <class T> constexpr int cap(T t) {
  auto [...xs] = t;
  auto f = [&] { return (xs + ...); };
  auto g = [=] { return (0 + ... + xs); };
  return f() + g();
}
static_assert(cap(P{1, 2, 3}) == 12);
template <class... Ts> constexpr int both(Ts... ts) {
  P p{1, 2, 3};
  auto [...xs] = p;
  return ((xs * ts) + ...);
}
static_assert(both(1, 10, 100) == 321);
template <class T> constexpr int nested(T t) {
  auto [...xs] = t;
  return [](auto... ys) { return sizeof...(ys); }(xs...);
}
static_assert(nested(P{}) == 3);
template <class T> struct S {
  template <class U> static constexpr int m(U u) { auto [...xs] = u; return sizeof...(xs) + sizeof(T); }
};
static_assert(S<char>::m(P{}) == 4);
