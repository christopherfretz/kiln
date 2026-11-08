// Debugger (WP-814): a namespace-scope variable, a static data member, and
// a lambda's by-copy and by-reference captures seen from inside its body.
//@ expect: debugger
//@ gdb: break globals_lambda.cpp:40
//@ gdb: run
//@ gdb: print g_count
//@ gdb: print Holder::total
//@ gdb: print scale
//@ gdb: print acc
//@ gdb: print k
//@ gdb: up
//@ gdb: info locals
//@ gdb-check: operator()(int) const (this=0x
//@ gdb-check: $1 = 3
//@ gdb-check: $2 = 11
//@ gdb-check: $3 = 2
//@ gdb-check: $4 = (int &) @0x
//@ gdb-check: $5 = 10
//@ gdb-check: #1
//@ gdb-check: acc = 1
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --file globals_lambda.cpp --line 40
//@ lldb: run
//@ lldb: target variable g_count Holder::total
//@ lldb: frame variable scale acc k
//@ lldb-check: stop reason = breakpoint 1.1
//@ lldb-check: (int) g_count = 3
//@ lldb-check: (int) Holder::total = 11
//@ lldb-check: (int) scale = 2
//@ lldb-check: (int &) acc = 0x
//@ lldb-check: (int) k = 10
int g_count = 3;
struct Holder {
  static int total;
};
int Holder::total = 11;
int run(int scale) {
  int acc = 1;
  auto add = [scale, &acc](int k) {
    acc += k * scale;
    return acc;
  };
  return add(10) + g_count + Holder::total;
}
int main() { return run(2) == 35 ? 0 : 1; }
