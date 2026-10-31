// [expr.prim.lambda.closure]/7, [temp.inst]/3: generic lambdas in function
// templates and class template members at run time: auto and explicit
// template parameters mixing the enclosing template's parameters, captures,
// a returned generic closure, a member template's closure, nested generic
// lambdas, function parameter packs (fold, sizeof..., forwarding), the
// conversion to pointer to function, and a constant-evaluated call.
//@ expect: run
//@ stdout: 42 3 7 10 5 4
//@ stdout: 12 18 7 7 4 9 5
//@ stdout: 16 3 12
extern "C" int printf(const char *, ...);
template <class T> T twice(T v) { return [](auto x) { return x + x; }(v); }
template <class T> T add(T v) { return [v](auto x, T y) { return x + y + v; }(1, v); }
template <class T> T tp(T v) { return []<class U>(U a, T b) { return T(a) * b; }(2, v); }
template <class T> struct Box {
  T val;
  T get() const { return [this](auto k) { return val + k; }(1); }
};
template <class T> constexpr T ce(T v) { return [](auto x) { return x * 2; }(v); }
static_assert(ce(4) == 8);
template <class T> T conv(T v) { int (*p)(int) = [](auto x) { return x + 3; }; return p(v); }

template <class T> auto make(T v) { return [v](auto x) { return x * v; }; }
template <class T> struct W {
  template <class U> static auto pair(U u) { return [u](auto t) -> decltype(auto) { return T(t) + U(u); }; }
};
template <class T> T nest(T v) {
  return [v](auto a) { return [v, a](auto b) { return v + a + b; }(T(1)); }(T(2));
}
template <class T> T ref(T v) { auto f = [&v](auto d) { v += d; }; f(1); f(2L); return v; }
template <class T> int sz(T) { return [](auto x) { return int(sizeof(x) + sizeof(T)); }('c'); }

template <class T> T sum(T a) { return [a](auto... xs) { return (a + ... + xs); }(1, 2, 3); }
template <class T> int cnt(T) { return [](auto... xs) { return int(sizeof...(xs)); }(1, 'a', 2.0); }
template <class T> T fwd(T a) { return [](auto f, auto... xs) { return f(xs...); }([](int x, int y) { return x * y; }, a, 3); }

int main() {
  Box<int> b{4};
  printf("%d %g %d %d %d %d\n", twice(21), twice(1.5), add(3), tp(5), b.get(), conv(1));
  auto m = make(3);
  printf("%d %g ", m(4), m(6.0));
  printf("%d ", W<int>::pair(2)(5));
  printf("%d %d ", nest(4), ref(1));
  printf("%d %d\n", sz(1.0), sz(1));
  printf("%d %d %d\n", sum(10), cnt(0), fwd(4));
}
