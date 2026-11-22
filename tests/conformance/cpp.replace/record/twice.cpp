// [cpp.subst]: a parameter used twice. Both `L` tokens have origin
// argument 0 with different uses (body tokens 0 and 2) and the same file
// location before substitution (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: twice.record
namespace L { int v; }
#define TWICE(x) x + x
int r = TWICE(L::v);
