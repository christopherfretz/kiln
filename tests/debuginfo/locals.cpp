// Debugger (WP-814): a breakpoint on a file:line, the backtrace, and the
// values of a parameter, a local, a struct member, a scoped enumerator, a
// reference and a class template specialization's member.
//@ expect: debugger
//@ gdb: break locals.cpp:48
//@ gdb: run
//@ gdb: bt
//@ gdb: print v
//@ gdb: print p.y
//@ gdb: print c
//@ gdb: print r
//@ gdb: print b.v[1]
//@ gdb: info locals
//@ gdb-check: Breakpoint 1, use (v=7, p=..., c=Color::Green, r=@0x
//@ gdb-check: #0  use (v=7, p=..., c=Color::Green, r=@0x
//@ gdb-check: in main () at tests/debuginfo/locals.cpp:53
//@ gdb-check: $1 = 7
//@ gdb-check: $2 = 2
//@ gdb-check: $3 = Color::Green
//@ gdb-check: $4 = (int &) @0x
//@ gdb-check: $5 = 6
//@ gdb-check: b = {v = {5, 6}}
//@ gdb-check: local = 9
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --file locals.cpp --line 48
//@ lldb: run
//@ lldb: bt
//@ lldb: frame variable v p.y c r b.v[1]
//@ lldb-check: stop reason = breakpoint 1.1
//@ lldb-check: `main at locals.cpp:53
//@ lldb-check: (int) v = 7
//@ lldb-check: (int) p.y = 2
//@ lldb-check: (Color) c = Green
//@ lldb-check: (int &) r = 0x
//@ lldb-check: (long) b.v[1] = 6
enum class Color { Red, Green };
struct P {
  int x;
  int y;
};
template <typename T> struct Box {
  T v[2];
};
int g = 3;
int use(int v, P p, Color c, int &r) {
  Box<long> b{{5, 6}};
  int local = v + p.y;
  local += (int)c + r + (int)b.v[1];
  return local;
}
int main() {
  P p{1, 2};
  return use(7, p, Color::Green, g) == 0;
}
