// C23 6.2.5 / [basic.fundamental]/1, [class.mem]: a `_BitInt(N)` whose
// LLVM `iN` allocates more bytes than its C++ size (N = 129: `i129`
// allocates 32 bytes, the type is 24; N = 300: 48 against 40) is the byte
// array `[size x i8]` in memory, as a record member, an array element, a
// local, a global and an indirect argument, and is loaded and stored as the
// integer of its size (`i192`, `i320`); where `iN` fits exactly
// (`_BitInt(200)`, `i256`) the member stays an integer, which can make the
// record packed. clang `typeRequiresSplitIntoByteArray`. Where
// BITINT_MAXWIDTH allows (x86_64).
#if __BITINT_MAXWIDTH__ >= 300
using u129 = unsigned _BitInt(129);
using s300 = _BitInt(300);
struct S {
  char c;
  u129 u;
  int x;
};
struct P {
  char c;
  _BitInt(200) b;
  u129 u;
  int x;
};
struct T {
  u129 a;
  s300 b;
};
S s;
P p;
T gt = {1, -2};
u129 gu = 3;
u129 arr[2];
s300 gneg = -(s300(1) << 200);
int f(S *q) {
  q->u = 7;
  return (int)q->u + (int)gu + (int)arr[1] + (int)p.b;
}
u129 id(u129 a) {
  u129 x = a;
  u129 l[3] = {1, 2, 3};
  return x + l[a & 1];
}
void h(u129 *q, int i, s300 v) {
  q[i] = 1;
  gt.b = v;
}
#endif
