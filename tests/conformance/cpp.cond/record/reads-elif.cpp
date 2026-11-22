// [cpp.cond]/13: `#if A` with `-DA=1` is taken, reading `A` bound to the
// command-line definition; the `#elif` after it is not evaluated and has
// no reads, nor has the `#else` (spec/pp.md "Preprocessing record",
// condition reads).
//@ expect: compile-ok
//@ strict-diags
//@ args: -DA=1
//@ pp-record: reads-elif.record
#if A
int a;
#elif B
int b;
#else
int c;
#endif
