//@ args: -O1 -Xclang -disable-llvm-passes
// `__builtin_unpredictable` above -O0 (a clang builtin;
// [intro.compliance.general]/11 extensions): the value is the operand,
// and a conditional branch whose condition is the call (parentheses
// stripped first, then implicit conversions: clang `EmitBranchOnBoolExpr`)
// or a `switch` whose condition is the call itself (`EmitSwitchStmt`)
// carries `!unpredictable !{}`. Loop conditions, a `select` and a call
// in parentheses under a conversion carry none.
void g(int);
void if_stmt(bool b) { if (__builtin_unpredictable(b)) g(1); }
void if_long(long b) { if (__builtin_unpredictable(b)) g(1); }
void if_not(bool b) { if (!__builtin_unpredictable(b)) g(1); }
void if_inner_paren(bool b) { if (__builtin_unpredictable((b))) g(1); }
void if_outer_paren(bool b) { if ((__builtin_unpredictable(b))) g(1); }
void if_compare(long b) { if (__builtin_unpredictable(b) == 1) g(1); }
void if_and(bool b, bool y) { if (__builtin_unpredictable(b) && y) g(1); }
void if_or(bool b, bool y) { if (y || __builtin_unpredictable(b)) g(1); }
void if_cond_var(bool b) { if (bool z = __builtin_unpredictable(b)) g(1); }
int cond_branch(bool b, int x, int y) { return __builtin_unpredictable(b) ? x : y; }
int cond_select(bool b) { return __builtin_unpredictable(b) ? 1 : 2; }
bool scalar_and(bool b, bool y) { return __builtin_unpredictable(b) && y; }
void switch_call(char c) {
  switch (__builtin_unpredictable(c)) { case 1: g(1); break; case 5: g(2); break; }
}
void switch_paren(long c) {
  switch ((__builtin_unpredictable(c))) { case 1: g(1); break; case 5: g(2); break; }
}
void loops(int x, int y) {
  while (__builtin_unpredictable(x--)) g(1);
  for (; __builtin_unpredictable(y); --y) g(2);
  do g(3); while (__builtin_unpredictable(y++ < 9));
}
template <class T> void dependent(T b) { if (__builtin_unpredictable(b)) g(1); }
template void dependent(int);
