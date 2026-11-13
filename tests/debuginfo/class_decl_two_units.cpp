// Debugger (WP-1389): under limited -g (the default) a class this unit
// defines (through the shared header) but only names through a pointer,
// never requiring it complete, is described as a declaration
// (DIFlagFwdDecl), as clang's `isCompleteDefinitionRequired` rule does;
// the debugger completes it from the unit that requires it
// (Inputs/class_decl_defs.cpp), so `ptype` and `print *w` still work here.
//@ expect: debugger
//@ args: -I%{test_dir}/Inputs %{test_dir}/Inputs/class_decl_defs.cpp
//@ gdb: break class_decl_two_units.cpp:31
//@ gdb: run
//@ gdb: ptype w
//@ gdb: print *w
//@ gdb: print w->a + w->b
//@ gdb-check: Breakpoint 1, main () at
//@ gdb-check: type = struct gadgets::Widget {
//@ gdb-check: int a;
//@ gdb-check: int b;
//@ gdb-check: } *
//@ gdb-check: $1 = {a = 3, b = 4}
//@ gdb-check: $2 = 7
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --file class_decl_two_units.cpp --line 31
//@ lldb: run
//@ lldb: frame variable *w
//@ lldb-check: stop reason = breakpoint 1.1
//@ lldb-check: (gadgets::Widget) *w = (a = 3, b = 4)
#include "class_decl_widget.h"

int main() {
  gadgets::Widget *w = make_widget();
  int sum = widget_sum(w);
  return sum == 7 ? 0 : 1;
}
