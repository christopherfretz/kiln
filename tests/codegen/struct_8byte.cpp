// By-value class of 8 bytes (two ints): one INTEGER eightbyte (i64).
struct S8 { int a, b; };
S8 ext(int);
int take(S8 s) { return s.a + s.b; }
S8 pass(int x) { return ext(x); }
int use(int x) {
  S8 local = ext(x);
  return take(ext(x)) + take(pass(x + 1)) + local.b;
}
