// By-value class of 17 bytes: MEMORY class on x86-64 (byval, sret);
// indirect on AArch64.
struct S17 { long a, b; char c; };
S17 ext(long);
long take(S17 s) { return s.a + s.b + s.c; }
S17 pass(long x) { return ext(x); }
long use(long x) {
  S17 local = ext(x);
  return take(ext(x)) + take(pass(x)) + local.c;
}
