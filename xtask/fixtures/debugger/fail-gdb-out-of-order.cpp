// Checks match successive lines in order: `$1` printed before `$2`.
//@ expect: debugger
//@ gdb: break fail-gdb-out-of-order.cpp:10
//@ gdb: run
//@ gdb: print 1
//@ gdb: print 2
//@ gdb-check: $2 = 2
//@ gdb-check: $1 = 1
int main() {
  return 0;
}
