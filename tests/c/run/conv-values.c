// C conversions that compute values: comparisons, `!`, `&&`, `||` yield
// int (C17 6.5.8p6, 6.5.3.3p5); `_Bool` and enumerations increment and
// decrement (6.5.2.4); enumeration constants are int unless int cannot
// represent them (clang `ActOnEnumBody`); `void *` converts implicitly
// (6.3.2.3p1); a character constant is an int (6.4.4.4p10).
//@ expect: run
//@ stdout: 1 0 2 4 | 1 1 0 1 | 2 3 1 | 4294967296 -1 8 | 7 65 4 1 | 4 2 2 2
int printf(const char *, ...);
enum color { RED, GREEN, BLUE };
enum big { SMALL = 1, HUGE = 0x100000000 };
enum mixed { NEG = -1, WIDE = 0x80000000u };
int g1 = (1 < 2) + (3 == 3), g2 = !0 + !5 + (2 && 1);
static int sum(void *p, int n) {
  int *ip = p, s = 0;
  for (int i = 0; i < n; i++) s += ip[i];
  return s;
}
int main(void) {
  int x = 3, y = 4;
  printf("%d %d %d %d | ", x < y, !x, (x < y) + (y > x), (int)sizeof(x == y));
  _Bool b = 0;
  b++;
  _Bool c = 1;
  c--;
  _Bool d = 0;
  d--;
  _Bool e = 1;
  ++e;
  printf("%d %d %d %d | ", b, (int)sizeof(b + b) == 4, c, d && e);
  enum color k = RED;
  k++;
  ++k;
  enum color m = k;
  m--;
  printf("%d %d %d | ", k, BLUE + 1, m == GREEN);
  printf("%lld %lld %d | ", (long long)HUGE, (long long)NEG, (int)sizeof(WIDE));
  int arr[3] = {1, 2, 4};
  void *vp = arr;
  int *back = vp;
  printf("%d %d %d %d | ", sum(vp, 3), 'A', (int)sizeof('A'), back == arr);
  _Complex double z = 1.0, w = 2.0;
  int *np = 0;
  printf("%d %d %d %d\n", g1 + g2, (z == w) + (z != w) * 2, (np == 0) + (arr + 1 > arr), (x && y) + (x || 0));
  return 0;
}
