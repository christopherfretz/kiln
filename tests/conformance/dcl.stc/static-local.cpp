// [dcl.stc] + [stmt.dcl]: a block-scope variable with static storage
// duration is initialized once and keeps its value between calls.
//@ expect: run
//@ stdout: 1 2 3 | 101 102
extern "C" int printf(const char*, ...);

int counter() {
  static int n = 0;
  return ++n;
}
int other() {
  static int n = 100;
  return ++n;
}

int main() {
  int a = counter(), b = counter(), c = counter();
  int d = other(), e = other();
  printf("%d %d %d | %d %d\n", a, b, c, d, e);
}
