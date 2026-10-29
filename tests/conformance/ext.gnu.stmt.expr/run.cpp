// GNU statement expressions at run time and in constant evaluation: the
// statements run in order, the last expression statement is the value,
// labels before it are jump targets, locals are scoped to the braces.
//@ expect: run
//@ stdout: 3 7 9 10 11 4 1
extern "C" int printf(const char*, ...);

constexpr int twice_plus_one(int x) {
  return ({ int y = x * 2; y + 1; });
}
static_assert(twice_plus_one(3) == 7);

int g;
int bump() { return ++g; }

int main() {
  static_assert(({ 2; }) * twice_plus_one(1) == 6);
  int a = ({ int y = 1; y + 2; });
  int b = twice_plus_one(a);
  int n = 0;
  int c = ({
    again:
      ++n;
      if (n < 3) goto again;
    n + 6;
  });
  int d = ({ int t = c; ({ t + 1; }); });
  ({ g = 10; (void)0; });
  int e = ({ bump(); ; });
  int *p = ({ static int s[2] = {4, 5}; s; });
  bool f = ({ g > 0; });
  printf("%d %d %d %d %d %d %d\n", a, b, c, d, e, *p, f);
  return 0;
}
