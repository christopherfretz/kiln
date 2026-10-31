// [expr.prim.lambda.closure]/8: the static invoker forwards its parameters
// and the (class-typed) result of the call operator.
//@ expect: run
//@ stdout: 1 2 3 4 | 10 20 | 6
extern "C" int printf(const char*, ...);
struct Big { long a, b, c, d; };
struct Pair { int x, y; };
int main() {
  Big (*mk)(long) = [](long s) { return Big{s, s + 1, s + 2, s + 3}; };
  Pair (*sw)(Pair) = [](Pair p) { return Pair{p.y, p.x}; };
  int (*rf)(const int&, int&&) = [](const int& a, int&& b) { return a + b; };
  Big b = mk(1);
  Pair p = sw(Pair{20, 10});
  printf("%ld %ld %ld %ld | %d %d | %d\n", b.a, b.b, b.c, b.d, p.x, p.y, rf(2, 4));
}
