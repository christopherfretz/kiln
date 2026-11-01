// [stmt.ranged] with a structured binding declaration ([dcl.struct.bind])
// as the for-range-declaration.
//@ expect: run
//@ stdout: 9
extern "C" int printf(const char*, ...);

struct P { int a, b; };

int main() {
  P ps[] = {{1, 2}, {3, 3}};
  int s = 0;
  for (auto [a, b] : ps) s += a + b;
  printf("%d\n", s);
}
