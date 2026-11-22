// [cpp.cond]/13: after a taken group, the conditions of the later `#elif`
// groups are not evaluated, and the `#else` group is not taken (spec/pp.md
// "Preprocessing record", conditionals).
//@ expect: compile-ok
//@ strict-diags
//@ args: -DA=1
//@ pp-record: elif-not-evaluated.record
#if A
int a;
#elif B
int b;
#else
int c;
#endif
