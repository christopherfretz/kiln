// `__builtin_unpredictable` at -O0 (a clang builtin;
// [intro.compliance.general]/11 extensions): the value is the operand and
// no branch or `switch` carries `!unpredictable` (clang adds the hint only
// when optimizing; `builtin_unpredictable_o1.cpp`).
void g(int);
void if_stmt(bool b) { if (__builtin_unpredictable(b)) g(1); }
int value(long x) { return __builtin_unpredictable(x) + 1; }
void switch_call(char c) {
  switch (__builtin_unpredictable(c)) { case 1: g(1); break; case 5: g(2); break; }
}
