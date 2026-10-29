// [expr.log.and] + [expr.log.or]: the second operand is not evaluated when
// the first determines the result.
//@ expect: run
//@ stdout: 0 1 1 0
//@ stdout: evaluated 2
extern "C" int printf(const char*, ...);

int count = 0;
bool yes() {
  ++count;
  return true;
}

int main() {
  bool a = false && yes();
  bool b = true || yes();
  bool c = true && yes();
  bool d = false || !yes();
  printf("%d %d %d %d\n", a, b, c, d);
  printf("evaluated %d\n", count);
}
