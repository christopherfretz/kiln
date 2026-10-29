// By-value class of 9 bytes: two INTEGER eightbytes ({ i64, i8 } on
// x86-64, [2 x i64] on AArch64).
struct S9 { long a; char b; };
S9 ext(long);
long take(S9 s) { return s.a + s.b; }
S9 pass(long x) { return ext(x); }
long use(long x) {
  S9 local = ext(x);
  return take(ext(x)) + take(pass(x)) + local.b;
}
