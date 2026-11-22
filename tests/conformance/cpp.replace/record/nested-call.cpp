// [cpp.rescan]: two expansions of `G` named in `CALL`'s replacement list,
// with `CALL` as parent and ordinals 1 and 2 under it (spec/pp.md
// "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: nested-call.record
#define G(n) (n)
#define CALL(f) f(1) + f(2)
int a = CALL(G);
