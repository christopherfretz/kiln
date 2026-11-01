// WP-165: with -Wno-increment-bool, ++ on _Atomic(bool) stores true
// (pre-increment yields true; post-increment yields the old value).
//@ expect: run
//@ args: -Wno-increment-bool
//@ stdout: 0 1 1 1
extern "C" int printf(const char *, ...);
_Atomic(bool) ab;
int main() {
  int a = ab++;
  int b = ab;
  ab = false;
  int c = ++ab;
  int d = ab++;
  printf("%d %d %d %d\n", a, b, c, d);
}
