// By-value class of 32 bytes: MEMORY class (byval align 8, sret).
struct S32 { long v[4]; };
S32 ext(long);
long take(S32 s) { long t = 0; for (int i = 0; i < 4; ++i) t += s.v[i]; return t; }
S32 pass(long x) { return ext(x); }
long use(long x) {
  S32 local = ext(x);
  return take(ext(x)) + take(pass(x)) + local.v[3];
}
