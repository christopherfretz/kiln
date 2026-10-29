// All-float classes: SSE eightbytes on x86-64 (<2 x float>, float,
// double); homogeneous floating-point aggregates on AArch64.
struct F2 { float x, y; };
struct F3 { float x, y, z; };
struct F4 { float x, y, z, w; };
struct D2 { double x, y; };
struct D3 { double x, y, z; };
F2 f2(float); F3 f3(float); F4 f4(float); D2 d2(double); D3 d3(double);
float g2(F2 v) { return v.x + v.y; }
float g3(F3 v) { return v.x + v.y + v.z; }
float g4(F4 v) { return v.x * v.w; }
double h2(D2 v) { return v.x * v.y; }
double h3(D3 v) { return v.x + v.z; }
F3 p3(float a) { return f3(a); }
D2 pd2(double a) { return d2(a); }
double use(float a, double b) {
  return g2(f2(a)) + g3(f3(a)) + g4(f4(a)) + h2(d2(b)) + h3(d3(b)) + g3(p3(a)) + h2(pd2(b));
}
