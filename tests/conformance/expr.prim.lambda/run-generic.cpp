// [expr.prim.lambda.closure]/3-6: generic lambdas at run time: invented
// and explicit template parameters, captures, decltype(auto), a
// constrained parameter, a function parameter pack, and the conversion
// to pointer to function.
//@ expect: run
//@ stdout: 1 a 13 20 7 6 4 6
extern "C" int printf(const char*, ...);
template <class T> concept Small = sizeof(T) <= 4;
int main() {
  int k = 10;
  auto id = [](auto x) { return x; };
  auto add = [k](auto x, auto y) { return x + y + k; };
  auto ref = [&k](auto d) { k += d; };
  auto all = [=](auto x) { return x * k; };
  auto tp = []<class T>(T a, T b) { return a < b ? b : a; };
  auto cn = [](Small auto x) { return x + 1; };
  auto da = [](auto& x) -> decltype(auto) { return (x); };
  auto va = [](auto... xs) { return (0 + ... + xs); };
  ref(5);
  int z = 3;
  da(z) = 4;
  printf("%d %c %d %d %d %d %d %d\n", id(1), id('a'), int(add(1, 2L)), all(2), tp(3, 7), cn(5), z, va(1, 2, 3));
  constexpr auto sq = [](auto v) { return v * v; };
  static_assert(sq(4) == 16);
  int (*fp)(int) = id;
  return fp(0);
}
