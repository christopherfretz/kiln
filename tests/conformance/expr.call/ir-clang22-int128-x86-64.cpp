// [expr.call]/7 (a parameter is copy-initialized from its argument),
// [basic.fundamental]/2 (extended integer types), [dcl.type.bitint]: the
// x86-64 call lowering of a 128-bit integer at -fclang-compat=22
// (spec/abi.md §8.25). `__int128`, `_BitInt(128)` and a class whose first
// eightbyte's IR type is an `i128` are passed and returned as `i128`, so
// the seventh integer argument `x` takes a 16-byte-aligned stack slot;
// two `long`s and a bare `_BitInt(65)` stay `{ i64, i64 }`. Compared with
// clang 22's IR exactly.
//@ expect: ir
// clang 22 has no C++26 erroneous values (P2795R5, spec/codegen.md §7r).
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized
//@ requires: clang>=22
//@ requires: llvm>=22
struct Wrap { __int128 v; };
struct Pair { long a, b; };
struct Bits { unsigned _BitInt(128) v; };

__int128 seven(long a, long b, long c, long d, long e, long f, __int128 x) { return x + a + f; }
unsigned __int128 twice(unsigned __int128 a) { return a * 2; }
_BitInt(128) bit(_BitInt(128) a) { return a; }
_BitInt(65) narrow(_BitInt(65) a) { return a; }
Wrap wrap(Wrap w) { return w; }
Pair pair(Pair p) { return p; }
Bits bits(Bits b) { return b; }

__int128 caller(long v) {
  __int128 r = seven(1, 2, 3, 4, 5, 6, v) + twice(v) + bit(v) + narrow(v);
  r += wrap({v}).v + pair({v, v}).b + bits({1}).v;
  return r;
}
