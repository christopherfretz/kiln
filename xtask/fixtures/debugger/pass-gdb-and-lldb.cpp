// Both debuggers stop on a line and print a local.
//@ expect: debugger
//@ gdb: break pass-gdb-and-lldb.cpp:16
//@ gdb: run
//@ gdb: print k
//@ gdb-check: pass-gdb-and-lldb.cpp:16
//@ gdb-check: $1 = 42
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --file pass-gdb-and-lldb.cpp --line 16
//@ lldb: run
//@ lldb: frame variable k
//@ lldb-check: stop reason = breakpoint
//@ lldb-check: (int) k = 42
int main() {
  int k = 42;
  return k - 42;
}
