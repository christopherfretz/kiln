// [expr.pre.incr] + [expr.post.incr]: prefix forms yield the updated
// lvalue, postfix forms yield the old value.
//@ expect: run
//@ stdout: 5 6 6 7 7 6 5
//@ stdout: 1.5 2.5 c
extern "C" int printf(const char*, ...);

int main() {
  int i = 5;
  int a = i++;
  int b = i;
  int c = ++i - 1;
  int d = i;
  int e = i--;
  int f = i;
  int g = --i;
  printf("%d %d %d %d %d %d %d\n", a, b, c + 1 - 1, d, e, f, g);
  double x = 0.5;
  ++x;
  double y = x++;
  char ch = 'a';
  ch += 1;
  ++ch;
  printf("%.1f %.1f %c\n", y, x, ch);
}
