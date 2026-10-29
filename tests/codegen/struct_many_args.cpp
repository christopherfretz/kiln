// Register exhaustion: past the sixth integer or eighth SSE register,
// x86-64 passes a two-eightbyte class as byval in memory.
struct P { long a, b; };
struct D { double x, y; };
P ep(); D ed();
long many(long a, long b, long c, long d, long e, P p) { return a + b + c + d + e + p.a + p.b; }
double fl(double a, double b, double c, double d, double e, double f, double g, D h) {
  return a + b + c + d + e + f + g + h.x + h.y;
}
long many_use() { return many(1, 2, 3, 4, 5, ep()); }
double fl_use() { return fl(1, 2, 3, 4, 5, 6, 7, ed()); }
