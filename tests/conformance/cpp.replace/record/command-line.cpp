// [cpp.predefined]/4, [cpp.replace]: a `-D` macro is a command-line
// definition with no file: its key is its name and text, not an offset in
// `<built-in>`, so another `-D` does not change it (command-line-other.cpp)
// (spec/pp.md "Preprocessing record").
//@ expect: compile-ok
//@ strict-diags
//@ args: -DNS=L
//@ pp-record: command-line.record
namespace L { void f(); }
void g() { NS::f(); }
