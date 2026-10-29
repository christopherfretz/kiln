// [expr.cond]: the conditional operator evaluates only the chosen operand,
// and yields an lvalue when both operands are lvalues of the same type.
//@ expect: run
//@ stdout: 1 0 big
//@ stdout: 10 9
extern "C" int printf(const char*, ...);

int calls = 0;
int touch(int v) {
  ++calls;
  return v;
}

int main() {
  int r = 1 ? touch(1) : touch(2);
  int x = 1, y = 2;
  (r ? x : y) = 10;
  (r ? y : x) += 7;
  printf("%d %d %s\n", calls, calls - r, x > 5 ? "big" : "small");
  printf("%d %d\n", x, y);
}
