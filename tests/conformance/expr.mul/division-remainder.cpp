// [expr.mul]: integer division truncates toward zero and
// (a/b)*b + a%b == a.
//@ expect: run
//@ stdout: 3 1 -3 -1 -3 1 3 -1
//@ stdout: 42 2.5
extern "C" int printf(const char*, ...);

int main() {
  int x = 7, y = 2;
  printf("%d %d %d %d %d %d %d %d\n", x / y, x % y, -x / y, -x % y, x / -y, x % -y,
         -x / -y, -x % -y);
  printf("%d %.1f\n", 6 * x, 5.0 / 2);
}
