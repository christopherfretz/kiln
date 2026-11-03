// [expr.call]/7 (a parameter is copy-initialized from its argument),
// [class.temporary]/3 (trivially copyable class arguments may be passed in
// registers): the call lowering of clang 20+ at -fclang-compat=22
// (spec/abi.md §8.21, spec/codegen.md §6a). On AArch64 an empty class is
// passed as `i64` (clang 19: `i8`) and a class passed indirectly whose
// destructor is trivial gets `dead_on_return` (LLVM 21+); the module
// carries the clang 22 data layout. Compared with clang 22's IR exactly.
//@ expect: ir
// clang 22 has no C++26 erroneous values (P2795R5, spec/codegen.md §7r).
//@ args: -ftrivial-auto-var-init=uninitialized
//@ requires: clang>=22
//@ requires: llvm>=22
struct Empty {};
struct Big { long a, b, c, d, e; };
struct Dtor { long a, b, c, d, e; ~Dtor(); };

int take_empty(Empty, int x) { return x; }
long take_big(Big b) { return b.a + b.e; }
long take_dtor(Dtor d) { return d.c; }
Empty make_empty() { return Empty{}; }
int take_two(Empty, Empty, int x);

long caller(long v) {
  Empty e;
  long r = take_empty(e, 1) + take_two(make_empty(), e, 2);
  r += take_big(Big{v, v, v, v, v});
  r += take_dtor(Dtor{v, v, v, v, v});
  return r;
}
