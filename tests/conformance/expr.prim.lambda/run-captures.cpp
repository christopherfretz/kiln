// [expr.prim.lambda.capture]: captured entities at run time. Copies are
// taken when the lambda is evaluated; references see later changes;
// `mutable` copies persist across calls; `this` and `*this` captures.
//@ expect: run
//@ stdout: 1 11 3 2 7 105 5 6 40 42
extern "C" int printf(const char*, ...);
struct S {
  int v;
  int get() { return [this] { return v; }(); }
  int copy() { auto l = [*this]() mutable { v += 100; return v; }; return l(); }
};
struct C {
  static int live;
  int id;
  C(int i) : id(i) { ++live; }
  C(const C& o) : id(o.id + 1) { ++live; }
  ~C() { --live; }
};
int C::live = 0;
int main() {
  int a = 1;
  auto byval = [a] { return a; };
  auto byref = [&a] { return a; };
  a = 11;
  int x = byval(), y = byref();
  auto counter = [n = 0]() mutable { return ++n; };
  counter();
  counter();
  int z = counter();
  int b = 2;
  [&] { b = [=] { return b; }(); }();
  S s{5};
  int g = s.get(), h = s.copy();
  int live;
  {
    C c(5);
    auto l = [c] { return c.id; };
    live = C::live;
    g += 0;
    int id = l();
    printf("%d %d %d %d %d %d %d %d ", x, y, z, b, live + id - 1 + 0 * g, h, g, id);
  }
  int arr[3] = {10, 20, 10};
  auto sum = [arr] { return arr[0] + arr[1] + arr[2]; };
  arr[0] = 0;
  printf("%d %d\n", sum(), [&arr] { return arr[1] * 2 + 2; }());
  return C::live;
}
