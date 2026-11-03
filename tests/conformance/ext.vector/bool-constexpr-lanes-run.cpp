// Clang ext vectors, bool element type, [clang 20+] ([expr.const]; clang
// VectorExprEvaluator, lvalue vector element access): a lane of a bool ext
// vector is read and written by subscript, also through a pointer, in
// loops and by compound assignment; whole vectors are copied and
// assigned, and lanes of bool vector members and array elements read
// back. WP-450.
//@ expect: run
//@ requires: clang>=22
typedef bool b3 __attribute__((ext_vector_type(3)));
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef bool b24 __attribute__((ext_vector_type(24)));
typedef unsigned char u8;

constexpr b8 A = {true, false, true, true, false, false, false, true};
static_assert(A[0] && !A[1] && A[2] && A[7]);

constexpr bool rd(int i) { return A[i]; }
static_assert(rd(3) && !rd(4));

constexpr int popc(b24 v) {
  int n = 0;
  for (int i = 0; i < 24; ++i) n += v[i];
  return n;
}
constexpr b24 every3() {
  b24 v = {};
  for (int i = 0; i < 24; i += 3) v[i] = true;
  return v;
}
static_assert(popc(every3()) == 8);
constexpr b24 M = every3();
static_assert(M[0] && M[21] && !M[22] && !M[23]);

constexpr int cnt(b16 v) {
  int c = 0;
  for (int i = 0; i < 16; ++i) c += v[i] ? 1 : 0;
  return c;
}
static_assert(cnt(__builtin_bit_cast(b16, (unsigned short)0xf0f1)) == 9);

constexpr bool flip(int i) { b8 v = A; v[i] = !v[i]; return v[i] && v[0]; }
static_assert(flip(1) && !flip(2));

constexpr bool lanes() {
  b8 v = {};
  v[3] |= true;
  v[3] &= true;
  v[4] ^= true;
  return v[3] && v[4] && !v[5];
}
static_assert(lanes());

constexpr bool whole() {
  b8 v = A;
  v &= !A;
  v |= A;
  v ^= A;
  b8 w;
  w = v;
  return !w[0] && !w[7] && __builtin_bit_cast(u8, w) == 0;
}
static_assert(whole());

constexpr bool copy3() {
  b3 a = {true, false, true};
  b3 b = a;
  b[1] = true;
  a = b;
  return a[0] && a[1] && a[2];
}
static_assert(copy3());

constexpr bool viaptr() {
  b8 v = {};
  b8 *p = &v;
  (*p)[6] = true;
  const b8 *q = p;
  return v[6] && (*q)[6] && !(*q)[5];
}
static_assert(viaptr());

constexpr bool cref() {
  b8 v = {true};
  const bool &r = v[0];
  v[0] = false;
  return r && !v[0];
}
static_assert(cref());

struct S { int x; b8 m; b3 t; };
constexpr S s = {7, {true, false, false, false, false, false, false, true}, {false, true, true}};
static_assert(s.m[7] && !s.m[1] && s.t[2] && !s.t[0]);

constexpr b8 arr[2] = {{true}, {false, true}};
static_assert(arr[0][0] && !arr[1][0] && arr[1][1]);

int main() {
  constexpr int n = popc(every3());
  constexpr bool ok = lanes() && whole() && viaptr();
  return n == 8 && ok ? 0 : 1;
}
