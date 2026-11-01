// GNU case ranges (`case lo ... hi`): fewer than 64 values become one
// switch case each; a wider range is tested in a `sw.caserange` block
// chained in front of the default destination (clang
// `EmitCaseStmtRange`); an empty range adds no case.
struct T { T(); ~T(); };
int g(int);
int f1(int x) {
  switch (x) {
  case 1 ... 3: return 1;
  case 5 ... 4: return 2;
  case 10: case 11 ... 12: case 13: return 3;
  case 100 ... 1000: g(1); [[fallthrough]];
  case 2000 ... 3000: return 5;
  case -50 ... -40: break;
  }
  return 0;
}
unsigned f2(unsigned x) {
  switch (x) {
  case 0xFFFFFF00u ... 0xFFFFFFFFu: return 1;
  case 7 ... 8: return 2;
  default: return 3;
  }
}
int f3(long x) {
  switch (x) {
  case 0 ... 63: g(0); break;
  case 64 ... 128: { T t; g(1); break; }
  }
  return 2;
}
int f4(char c) {
  switch (c) { case 'a' ... 'z': return 1; case '0' ... '9': return 2; }
  switch (T t; c) { case 1 ... 100: return 3; }
  return 0;
}
template <int N> int f5(int x) { switch (x) { case N ... N + 100: return 1; } return 0; }
int h() { return f5<3>(4); }
constexpr int f6(int x) { switch (x) { case 1 ... 9: return 1; case 10 ... 99: return 2; } return 3; }
static_assert(f6(5) == 1 && f6(50) == 2 && f6(500) == 3);
