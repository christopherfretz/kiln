// Clang extension: _Atomic(T) objects at run time: ++/--, every compound
// assignment (atomicrmw or a cmpxchg loop), conversions through the value
// type, float/double/long double/__int128/bool/pointer objects, the bare
// qualifier and _Atomic members.
//@ expect: run
//@ stdout: 5 7 7 5
//@ stdout: 9
//@ stdout: -4
//@ stdout: 9
//@ stdout: 0 1
//@ stdout: 6.5 0.75
//@ stdout: 6 10
//@ stdout: 2
//@ stdout: 12 13
//@ stdout: 9 1
extern "C" int printf(const char *, ...);
_Atomic(int) gi = 5;
_Atomic(short) gs = -2;
_Atomic(unsigned) gu = 10;
_Atomic(bool) gb = true;
_Atomic(float) gf = 1.5f;
_Atomic(double) gd = 2.0;
_Atomic(long double) gld = 3.0L;
_Atomic(__int128) gw = 7;
_Atomic int gq = 3;
struct C { _Atomic(int) n; } gc;
int main() {
  int arr[3] = {1, 2, 3};
  _Atomic(int *) ip = arr;
  int a = gi++, b = ++gi, c = gi--, d = --gi;
  printf("%d %d %d %d\n", a, b, c, d);
  gi += 3; gi -= 1; gi *= 2; gi /= 3; gi %= 4; gi <<= 3; gi >>= 1; gi &= 7; gi |= 8; gi ^= 1;
  printf("%d\n", (int)gi);
  gs += 1; gs *= 3; gs -= 1;
  printf("%d\n", (int)gs);
  gu -= 2.5; gu += 1;
  printf("%u\n", (unsigned)gu);
  gb = !gb; bool b1 = gb; gb += 1;
  printf("%d %d\n", b1, (int)gb);
  gf++; ++gf; gf *= 2; gf -= 0.5f;
  gd += 1.5; gd /= 2; gd--;
  printf("%g %g\n", (double)gf, (double)gd);
  gld = gld * 2; gw = gw + 1; gw += 2;
  printf("%g %d\n", (double)gld, (int)gw);
  ip++; ++ip; ip--;
  printf("%d\n", *ip);
  gq += gi; gc.n = gq; gc.n++;
  int local = gc.n;
  printf("%d %d\n", (int)gq, local);
  _Atomic(int) x = 4, y = 9;
  x = y; y = 1;
  printf("%d %d\n", (int)x, (int)y);
}
