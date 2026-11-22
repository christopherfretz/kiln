// [cpp.cond]/13: an `#elif` chain whose middle group is taken: the groups
// before it are not taken, the `#elif` after it is not evaluated and the
// `#else` is not taken; the groups tile the conditional (spec/pp.md
// "Preprocessing record", conditionals).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: elif-chain.record
#define TWO 2
#if TWO == 0
int zero;
#elifdef NOPE
int nope;
#elif TWO == 2
int two;
#elif TWO == 3
int three;
#else
int other;
#endif
