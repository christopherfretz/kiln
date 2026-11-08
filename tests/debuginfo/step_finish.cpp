// Debugger (WP-814): `step` into a method defined in its class, `finish`
// back to the caller with the returned value, and `next` over a call.
//@ expect: debugger
//@ gdb: break step_finish.cpp:38
//@ gdb: run
//@ gdb: step
//@ gdb: bt
//@ gdb: finish
//@ gdb: next
//@ gdb: next
//@ gdb-check: Breakpoint 1, main () at
//@ gdb-check: Counter::bump (this=0x
//@ gdb-check: n += by;
//@ gdb-check: #0  Counter::bump (this=0x
//@ gdb-check: main () at tests/debuginfo/step_finish.cpp:38
// (gdb -batch does not print finish's "Run till exit" banner.)
//@ gdb-check: Value returned is $1 = 5
//@ gdb-check: got += c.bump(1);
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --file step_finish.cpp --line 38
//@ lldb: run
//@ lldb: step
//@ lldb: finish
//@ lldb-check: stop reason = breakpoint 1.1
//@ lldb-check: stop reason = step in
//@ lldb-check: `Counter::bump(this=0x
//@ lldb-check: stop reason = step out
//@ lldb-check: Return value: (int) $0 = 5
struct Counter {
  int n = 4;
  int bump(int by) {
    n += by;
    return n;
  }
};
int main() {
  Counter c;
  int got = c.bump(1);
  got += c.bump(1);
  return got == 11 ? 0 : 1;
}
