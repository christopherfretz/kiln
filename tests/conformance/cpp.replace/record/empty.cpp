// [cpp.replace]: expansions to nothing have no entry; the argument of
// `EAT` still spans `L::T`, and `x` has no use (spec/pp.md "Preprocessing
// record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: empty.record
#define EAT(x)
EAT(L::T)
#define NOTHING
NOTHING
int i;
