// [basic.start.main]: returning from main is like calling exit with the
// return value; it need not be a constant.
//@ expect: run
//@ stdout: leaving
//@ exit: 42
extern "C" int printf(const char*, ...);

int compute(int a) { return a * 6; }

int main() {
  printf("leaving\n");
  return compute(7);
}
