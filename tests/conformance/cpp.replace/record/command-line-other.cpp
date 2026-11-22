// [cpp.predefined]/4: command-line.cpp with one more, unrelated `-D`: the
// definition of `NS` has the same key (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ args: -DOTHER=2 -DNS=L
//@ pp-record: command-line-other.record
namespace L { void f(); }
void g() { NS::f(); }
