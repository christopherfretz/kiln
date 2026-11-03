// [basic.fundamental]/2 (extended integer types), [dcl.type.bitint]
// (C23 _BitInt), [expr.call]/7: the x86-64 call lowering of a 128-bit
// integer (spec/abi.md §8.25). clang 20+ (`-fclang-compat=22`) passes and
// returns `__int128`, `_BitInt(128)`, an enum with such an underlying type
// and a class whose first eightbyte's IR type is an `i128` as `i128`, so a
// stack-passed one takes a 16-byte-aligned slot; clang 19 splits them into
// `{ i64, i64 }` / two `i64`s. Two `long`s stay `{ i64, i64 }`. AArch64
// passes `i128` at every level. Compared with clang 19 and clang 22.
__int128 ret(__int128 a) { return a; }
unsigned __int128 uret(unsigned __int128 a) { return a + 1; }
// Six integer registers are taken, so `x` is passed on the stack.
__int128 seven(long a, long b, long c, long d, long e, long f, __int128 x) { return x + a + f; }
// One register left: `x` goes to the stack, `y` would fit but follows it.
__int128 split(long a, long b, long c, long d, long e, __int128 x, long y) { return x + y + a + e; }
_BitInt(128) bi(_BitInt(128) a) { return a; }
unsigned _BitInt(128) ubi(unsigned _BitInt(128) a) { return a; }
_BitInt(65) b65(_BitInt(65) a) { return a; }
enum E : __int128 { A = 1 };
E en(E e) { return e; }
struct S1 { __int128 v; };
S1 s1(S1 s) { return s; }
struct S2 { long a, b; };
S2 s2(S2 s) { return s; }
struct S3 { unsigned _BitInt(128) v; };
S3 s3(S3 s) { return s; }
struct S4 { S1 inner; };
S4 s4(S4 s) { return s; }
union U { __int128 x; long y; };
U u(U s) { return s; }
struct Arr { __int128 v[1]; };
Arr arr(Arr s) { return s; }
S1 s1_stack(long a, long b, long c, long d, long e, long f, S1 s) { return s; }

__int128 call(long v) {
  __int128 r = seven(1, 2, 3, 4, 5, 6, v) + split(1, 2, 3, 4, 5, v, 7);
  r += ret(v) + uret(v) + bi(v) + ubi(v) + b65(v) + en(A);
  r += s1({v}).v + s2({v, v}).a + s3({1}).v + s4({{v}}).inner.v + u({v}).x;
  r += arr({{v}}).v[0] + s1_stack(1, 2, 3, 4, 5, 6, {v}).v;
  return r;
}
