// By-value class of 16 bytes: two INTEGER eightbytes ({ i64, i64 }).
struct S16 { long a, b; };
S16 ext(long);
long take(S16 s) { return s.a - s.b; }
S16 pass(long x) { return ext(x); }
long use(long x) {
  S16 local = ext(x);
  return take(ext(x)) + take(pass(x)) + local.b;
}
