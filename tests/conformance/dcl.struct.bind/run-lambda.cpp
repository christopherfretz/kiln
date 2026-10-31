//@ expect: run
//@ stdout: 3 1 2 13 4
// [expr.prim.lambda.capture] (C++20 P1091/P1381) at run time: a binding
// captured by reference aliases the enclosing object's member; one captured
// by copy holds the value at the capture.
extern "C" int printf(const char *, ...);
struct P { int x, y; };
int main() {
  P p{1, 2};
  auto [a, b] = p;
  auto sum = [&] { return a + b; };
  auto copy = [a] { return a; };
  auto setb = [&b] { b = 12; };
  int s1 = sum();
  setb();
  auto [c, d] = p;
  auto inc = [=]() mutable { return ++d + c; };
  printf("%d %d %d %d %d\n", s1, copy(), p.y, sum(), inc());
}
