// WP-583, [expr.call]/7, AAPCS64 §6.8.2 C.4/C.10 (clang
// `AArch64ABIInfo::classifyArgumentType`, `getTypeUnadjustedAlign`): an
// aggregate of at most 16 bytes is classified by its unadjusted (natural)
// alignment. The class's own `alignas`/`aligned` (or an `aligned` typedef
// of it) does not count, so `alignas(16)` two `i64`s are `[2 x i64]` in
// any register pair, not an even-pair `i128`; a member's `alignas`, an
// over-aligned member class, an `unsigned __int128` member or a bit-field
// of it do count (`i128`); a non-empty base counts from clang 21 on
// (llvm-project#135551: `DE` is `[2 x i64]` under clang 19, `i128` under
// 22), an empty base at offset 0 never does; `packed` and `#pragma pack`
// lower it. The same alignment picks an HFA's
// `alignstack` and the `va_arg` register/stack rounding; a return keeps
// clang's adjusted alignment (`i128`). On x86-64 only the size matters
// (an `alignas(32)` 8-byte class is 32 bytes: MEMORY, `byval align 32`).
// spec/abi.md §8.12 and spec/landed/abi/WP-583.md.
#include <stdarg.h>

struct alignas(16) A { unsigned long lo, hi; };
struct __attribute__((aligned(16))) B { unsigned long lo, hi; };
struct C { alignas(16) unsigned long lo; unsigned long hi; };
struct D { A a; };
struct E { unsigned __int128 v; };
struct F { long double v; };
struct alignas(16) G { float a, b, c, d; };
struct Q { G g; };
struct H { long lo, hi; };
typedef H H16 __attribute__((aligned(16)));
struct __attribute__((packed)) P { unsigned __int128 v; };
#pragma pack(push, 4)
struct K { unsigned __int128 v; };
#pragma pack(pop)
struct DE : E {};
struct alignas(16) EB {};
struct DEB : EB { long lo, hi; };
struct W { unsigned __int128 v : 100; };
struct alignas(16) I { unsigned long lo; };
struct alignas(32) X { long v; };

unsigned long fa(int x, A a) { return x + a.lo + a.hi; }
unsigned long fb(int x, B b) { return x + b.lo + b.hi; }
unsigned long fc(int x, C c) { return x + c.lo + c.hi; }
unsigned long fd(int x, D d) { return x + d.a.lo; }
unsigned long fe(int x, E e) { return x + (unsigned long)e.v; }
long double ff(int x, F f) { return x + f.v; }
float fg(int x, G g) { return x + g.a + g.d; }
float fq(int x, Q q) { return x + q.g.a; }
long fh(int x, H h) { return x + h.lo + h.hi; }
long fh16(int x, H16 h) { return x + h.lo; }
unsigned long fp(int x, P p) { return x + (unsigned long)p.v; }
unsigned long fk(int x, K k) { return x + (unsigned long)k.v; }
unsigned long fde(int x, DE d) { return x + (unsigned long)d.v; }
long fdeb(int x, DEB d) { return x + d.lo; }
unsigned long fw(int x, W w) { return x + (unsigned long)w.v; }
unsigned long fi(int x, I i) { return x + i.lo; }
long fx(int x, X v) { return x + v.v; }

A ra(unsigned long v) { return {v, v + 1}; }
C rc(unsigned long v) { return {v, v + 1}; }
G rg(float v) { return {v, v, v, v}; }
H rh(long v) { return {v, v + 1}; }
I ri(unsigned long v) { return {v}; }

unsigned long va(int n, ...) {
  va_list ap;
  va_start(ap, n);
  A a = va_arg(ap, A);
  C c = va_arg(ap, C);
  G g = va_arg(ap, G);
  Q q = va_arg(ap, Q);
  va_end(ap);
  return n + a.lo + c.hi + (unsigned long)g.a + (unsigned long)q.g.d;
}

unsigned long calls() {
  A a{7, 1};
  C c{7, 1};
  G g{1, 2, 3, 4};
  Q q{g};
  unsigned long r = fa(3, a) + fb(3, B{7, 1}) + fc(3, c) + fd(3, D{a}) + fe(3, E{5});
  r += (unsigned long)ff(3, F{2}) + (unsigned long)fg(3, g) + (unsigned long)fq(3, q);
  r += fh(3, H{7, 1}) + fh16(3, H16{7, 1}) + fp(3, P{5}) + fk(3, K{5}) + fde(3, DE{});
  r += fdeb(3, DEB{});
  r += fw(3, W{5}) + fi(3, I{7}) + fx(3, X{7});
  r += ra(1).lo + rc(1).hi + (unsigned long)rg(1).b + rh(1).lo + ri(1).lo;
  return r + va(3, a, c, g, q);
}
