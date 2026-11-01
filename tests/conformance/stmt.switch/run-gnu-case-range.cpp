// GNU case ranges (`case lo ... hi`): narrow ranges are expanded to case
// values, wide ones tested against the condition before the default
// (clang `EmitCaseStmtRange`); constant evaluation matches the same.
//@ expect: run
//@ strict-diags
int cls(long x) {
  switch (x) {
  case 1 ... 3: return 1;
  case 10: case 11 ... 12: return 2;
  case 100 ... 100000: return 3;
  case -500 ... -200: return 4;
  default: return 0;
  }
}
unsigned ucls(unsigned x) {
  switch (x) {
  case 0xFFFFFF00u ... 0xFFFFFFFFu: return 1;
  case 0 ... 0x7Fu: return 2;
  }
  return 3;
}
int dtor_count = 0;
struct S { ~S() { ++dtor_count; } };
int scoped(int x) {
  switch (S s; x) {
  case 0 ... 99: { S t; return 1; }
  case 100 ... 1000: break;
  }
  return 2;
}
template <int L> constexpr int tcls(int x) { switch (x) { case L ... L + 100: return 1; } return 0; }
static_assert(tcls<5>(5) == 1 && tcls<5>(105) == 1 && tcls<5>(106) == 0 && tcls<5>(4) == 0);
int main() {
  const long in[] = {0, 1, 3, 4, 10, 12, 13, 99, 100, 5000, 100000, 100001, -500, -200, -199, -501};
  const int out[] = {0, 1, 1, 0, 2, 2, 0, 0, 3, 3, 3, 0, 4, 4, 0, 0};
  for (int i = 0; i < 16; ++i) if (cls(in[i]) != out[i]) return 1 + i;
  if (ucls(0xFFFFFFFFu) != 1 || ucls(0xFFFFFF00u) != 1 || ucls(0xFFFFFEFFu) != 3) return 20;
  if (ucls(0) != 2 || ucls(0x7F) != 2 || ucls(0x80) != 3) return 21;
  if (scoped(5) != 1 || dtor_count != 2) return 22;
  if (scoped(500) != 2 || dtor_count != 3) return 23;
  if (scoped(5000) != 2 || dtor_count != 4) return 24;
  if (tcls<1000>(1050) != 1) return 25;
  return 0;
}
