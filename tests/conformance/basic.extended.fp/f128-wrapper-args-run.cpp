// [basic.extended.fp], x86-64 SysV 3.2.3 (classification): a struct whose
// only member is a 128-bit float (or a one-element array of one, or such a
// struct behind an empty base) is SSE+SSEUP and travels in one XMM
// register as `fp128`; a 16-byte vector wrapper likewise as the vector.
// On AArch64 the same structs are homogeneous aggregates of one. Checks
// that values survive passing and returning through non-inlined calls,
// in both directions, mixed with integer and other float arguments
// (the IR form is diffed against clang in tests/codegen/fp128_wrapper_abi.cpp).
//@ expect: run
//@ stdout: 1.5 -2.25 3 0.125 10
using F = decltype(__builtin_inff128());
typedef float v4 __attribute__((vector_size(16)));
struct Q { F x; };
struct A { F x[1]; };
struct E {};
struct N : E { Q q; };
struct V { v4 v; };
extern "C" int printf(const char *, ...);
[[gnu::noinline]] Q pass(Q q) { return q; }
[[gnu::noinline]] A neg(int, A a, double) { return A{{-a.x[0]}}; }
[[gnu::noinline]] N add(N a, long k, N b) { return N{{}, Q{a.q.x + b.q.x + F(k)}}; }
[[gnu::noinline]] V swap(V v) { return V{{v.v[3], v.v[2], v.v[1], v.v[0]}}; }
[[gnu::noinline]] F sum(Q a, Q b, Q c, Q d, Q e, Q f, Q g, Q h, Q i, Q j) {
  return a.x + b.x + c.x + d.x + e.x + f.x + g.x + h.x + i.x + j.x;
}
int main() {
  Q q = pass(Q{F(1.5)});
  A a = neg(1, A{{F(2.25)}}, 0.5);
  N n = add(N{{}, Q{F(1)}}, 1, N{{}, Q{F(1)}});
  V v = swap(V{{0.125f, 2, 3, 4}});
  Q o{F(1)};
  // Ten wrappers: the last two spill to the stack after eight registers.
  F s = sum(o, o, o, o, o, o, o, o, Q{F(1)}, Q{F(1)});
  printf("%g %g %g %g %g\n", double(q.x), double(a.x[0]), double(n.q.x), double(v.v[3]),
         double(s));
}
