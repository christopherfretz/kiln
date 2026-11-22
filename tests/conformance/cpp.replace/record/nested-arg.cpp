// [cpp.subst]/1: an invocation in an argument. The inner `ID`'s name is in
// the file and its parent is the outer one; `L` is argument 0 of the
// outer, located at argument 0 of the inner, located in the file (spec/pp.md
// "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ pp-record: nested-arg.record
namespace L { int v; }
#define ID(x) x
int r = ID(ID(L::v));
