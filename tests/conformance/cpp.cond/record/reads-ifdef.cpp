// [cpp.cond]/14: `#ifdef`, `#ifndef`, `#elifdef` and `#elifndef` each read
// exactly one name, at its token, bound or "undefined" (spec/pp.md
// "Preprocessing record", condition reads).
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: reads-ifdef.record
#define Z 1
#ifdef X
int x;
#elifdef Y
int y;
#elifndef Z
int z;
#else
int other;
#endif
#ifndef X
int not_x;
#endif
