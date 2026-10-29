// Calls whose struct result is ignored or used as a temporary member.
struct S16 { long a, b; };
struct S40 { long v[5]; };
S16 mk16(long);
S40 mk40(long);
long ignored(long x) {
  mk16(x);
  mk40(x);
  return mk16(x).b + mk40(x).v[4];
}
