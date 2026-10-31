// [class.eq]/3: a defaulted `==` called at run time (code generation of
// the implicit definition).
//@ expect: run
//@ stdout: 1 0 0 1 1
extern "C" int printf(const char*, ...);
struct B { int b; bool operator==(const B&) const = default; };
struct S : B {
  int a[3];
  double d;
  bool operator==(const S&) const = default;
};
struct W { S s; friend bool operator==(const W&, const W&) = default; };
int main(int argc, char**) {
  S x{{1}, {1, 2, 3}, 0.5}, y = x;
  y.a[2] = argc + 7;
  S z = x;
  z.b = argc + 1;
  W w1{x}, w2{x};
  printf("%d %d %d %d %d\n", x == x, x == y, x == z, w1 == w2, !(w1 != w2));
  return 0;
}
