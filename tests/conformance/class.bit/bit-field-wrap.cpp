// [class.bit]: a bit-field holds values of its declared width; storing a
// value out of range of an unsigned bit-field wraps modulo 2^width.
//@ expect: run
//@ stdout: 7 0 5 3 -2
extern "C" int printf(const char*, ...);

struct Flags {
  unsigned a : 3;
  unsigned b : 3;
  unsigned c : 4;
  int s : 3;
};

int main() {
  Flags f;
  unsigned eight = 8, nineteen = 19;
  int m = -2;
  f.a = 7;
  f.b = eight;
  f.c = 5;
  f.s = m;
  unsigned b0 = f.b;
  f.b = nineteen;
  printf("%u %u %u %u %d\n", f.a, b0, f.c, f.b, f.s);
}
