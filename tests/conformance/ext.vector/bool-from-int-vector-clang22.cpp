// Clang ext vectors, [clang 20+] (clang Sema IsVectorConversion,
// ICK_Boolean_Conversion, CK_IntegralToBoolean): an integer ext vector
// converts implicitly to a `bool` ext vector of the same lane count, each
// lane true when nonzero: in initialization, assignment, a return, a call
// argument (a Conversion-rank match, so an exact `i4` overload wins and a
// non-viable `f4` or `b8` overload loses), template deduction and casts.
// The result is stored on the packed `bool` vector layout. WP-453.
//@ expect: run
//@ requires: clang>=22
typedef int i4 __attribute__((ext_vector_type(4)));
typedef unsigned char uc4 __attribute__((ext_vector_type(4)));
typedef long l3 __attribute__((ext_vector_type(3)));
typedef short s16 __attribute__((ext_vector_type(16)));
typedef float f4 __attribute__((ext_vector_type(4)));
typedef bool b3 __attribute__((ext_vector_type(3)));
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b16 __attribute__((ext_vector_type(16)));

static int lanes(b4 v) { return v[0] | v[1] << 1 | v[2] << 2 | v[3] << 3; }

static int h(b4) { return 1; }
static int h(f4) { return 2; }
static int k(b4) { return 1; }
static int k(i4) { return 2; }
static int m(b8) { return 1; }
static int m(b4) { return 2; }

template <class T> b4 conv(T t) { return t; }

struct S {
  b4 v;
  S(b4 b) : v(b) {}
};

static b4 ret(uc4 u) { return u; }

int main() {
  i4 I = {0, 3, -1, 0};
  b4 a = I;
  if (lanes(a) != 0b0110) return 1;
  b4 c = I < 1;
  if (lanes(c) != 0b1101) return 2;
  a = I == 0;
  if (lanes(a) != 0b1001) return 3;
  if (lanes(ret(uc4{0, 0, 255, 1})) != 0b1100) return 4;
  l3 L = {5, 0, -7};
  b3 r;
  r = L;
  if (!r[0] || r[1] || !r[2]) return 5;
  s16 s = {};
  s[15] = 2;
  b16 w = static_cast<b16>(s);
  for (int i = 0; i < 15; ++i)
    if (w[i]) return 6;
  if (!w[15]) return 7;
  if (lanes((b4)I) != 0b0110) return 8;
  if (lanes(conv(I)) != 0b0110) return 9;
  if (h(I) != 1 || k(I) != 2 || m(I) != 2) return 10;
  S st(I);
  if (lanes(st.v) != 0b0110) return 11;
  bool x = conv(I)[1];
  return x ? 0 : 12;
}
