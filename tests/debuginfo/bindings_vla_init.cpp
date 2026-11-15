// Debugger (WP-1445): a structured binding's value (a member at an
// offset and a bit-field), a variable-length array's element, and a
// breakpoint inside a dynamic initializer's compiler-generated function.
//@ expect: debugger
//@ args: -Wno-vla-cxx-extension
//@ gdb: break bindings_vla_init.cpp:41
//@ gdb: break bindings_vla_init.cpp:48
//@ gdb: run
//@ gdb: bt
//@ gdb: continue
//@ gdb: print y
//@ gdb: print bz
//@ gdb: print vla[2]
//@ gdb-check: Breakpoint 1, __cxx_global_var_init
//@ gdb-check: Breakpoint 2, use
//@ gdb-check: $1 = 20
//@ gdb-check: $2 = 5
//@ gdb-check: $3 = 12
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --file bindings_vla_init.cpp --line 41
//@ lldb: breakpoint set --file bindings_vla_init.cpp --line 48
//@ lldb: run
//@ lldb: bt
//@ lldb: continue
//@ lldb: frame variable y bz vla[2]
//@ lldb-check: stop reason = breakpoint 1.1
//@ lldb-check: __cxx_global_var_init
//@ lldb-check: stop reason = breakpoint 2.1
//@ lldb-check: (long) y = 20
//@ lldb-check: bz = 5
//@ lldb-check: (int) vla[2] = 12
struct P {
  int x;
  long y;
};
struct B {
  int bx : 3;
  unsigned bz : 5;
};
int make(int v) { return v * 2; }
int global = make(21);
int use(P p, B b, int n) {
  auto [x, y] = p;
  auto [bx, bz] = b;
  int vla[n];
  for (int i = 0; i < n; ++i)
    vla[i] = i * 6;
  return x + (int)y + bx + (int)bz + vla[n - 1];
}
int main() { return use(P{1, 20}, B{1, 5}, 3) == global; }
