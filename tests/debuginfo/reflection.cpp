// Debugger (WP-910): reflection at `-g`. A class completed by
// `define_aggregate` ([meta.reflection.define.aggregate]) has its
// members' debug information, and a class template specialized on a
// `std::meta::info` value ([temp.arg.nontype], [basic.fundamental]/17)
// debugs like any other, its `std::meta::info` parameter a pointer-sized
// base type of that name with no value; a consteval-only variable emits
// nothing. In the default gcc reflection mangling (WP-909) gdb demangles
// `use`'s symbol (`_Z3use5point6HolderILDmtyiEE`) and names the frame
// `use (`; the kiln form's `u6__info` left it a raw signature.
//@ expect: debugger
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ gdb: break reflection.cpp:51
//@ gdb: run
//@ gdb: print p
//@ gdb: print h
//@ gdb: ptype p
//@ gdb: ptype h
//@ gdb-check: Breakpoint 1, use (
//@ gdb-check: $1 = {x = 3, y = 4}
//@ gdb-check: $2 = {value = 5}
//@ gdb-check: type = struct point {
//@ gdb-check:     int x;
//@ gdb-check:     long y;
//@ gdb-check: type = struct Holder<^^int> {
//@ gdb-check:     int value;
//@ lldb: settings set target.disable-aslr false
//@ lldb: breakpoint set --file reflection.cpp --line 51
//@ lldb: run
//@ lldb: frame variable p h sum
//@ lldb-check: stop reason = breakpoint 1.1
//@ lldb-check: (point) p = (x = 3, y = 4)
//@ lldb-check: h = (value = 5)
//@ lldb-check: (int) sum = 13
#include "kiln_meta.h"
using namespace std::meta;

struct point;
consteval {
  define_aggregate(^^point, {data_member_spec(^^int, {.name = "x"}),
                             data_member_spec(^^long, {.name = "y"})});
}

template <info R> struct Holder { typename[:R:] value; };

constexpr info gr = ^^int;

int use(point p, Holder<^^int> h) {
  constexpr bool same = gr == ^^int;
  int sum = p.x + static_cast<int>(p.y) + h.value + same;
  return sum;
}

int main() {
  point p{3, 4};
  Holder<^^int> h{5};
  return use(p, h) == 13 ? 0 : 1;
}
