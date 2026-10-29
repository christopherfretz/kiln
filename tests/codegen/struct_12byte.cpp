// 12 bytes of ints: { i64, i32 } on x86-64 (wider than the object, so a
// coercion temporary and memcpy), [2 x i64] on AArch64.
struct S12 { int a, b, c; };
S12 ext(int);
int take(S12 s) { return s.a + s.b + s.c; }
S12 pass(int x) { return ext(x); }
int use(int x) {
  S12 local = ext(x);
  return take(ext(x)) + take(pass(x)) + local.c;
}
