// Scalar and class arguments in one signature.
struct S8 { int a, b; };
struct S24 { long a, b, c; };
struct F2 { float x, y; };
S8 e8(); S24 e24(); F2 ef2();
long mixed(int i, S8 a, double d, S24 b, F2 f, char c) {
  return i + a.a + (long)d + b.c + (long)f.y + c;
}
long mixed_use(int x) { return mixed(x, e8(), 1.5, e24(), ef2(), 'c'); }
