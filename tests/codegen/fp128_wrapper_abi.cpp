// x86-64 SysV (clang X86_64ABIInfo::classifyArgumentType/ReturnType,
// GetByteVectorType): an SSE+SSEUP eightbyte pair passes as the vector or
// fp128 a single-element wrapper holds (isSingleElementStruct: through
// one-element arrays, nested records and empty bases), so a struct of one
// 128-bit float or one 16-byte vector is `fp128` / `<4 x float>`, not the
// struct type. On AArch64 they are an HFA / HVA of one, as before.
// extern "C" keeps clang's __float128 and kiln's _Float128 spellings of
// decltype(__builtin_inff128()) out of the IR (their manglings differ).
using F = decltype(__builtin_inff128());
typedef float v4 __attribute__((vector_size(16)));
struct Q { F x; };
struct A { F x[1]; };
struct E {};
struct N : E { Q q; };
struct V { v4 v; };
struct W { V w[1]; };
extern "C" {
Q q_id(Q q) { return q; }
A a_id(A a) { return a; }
N n_id(N n) { return n; }
V v_id(V v) { return v; }
W w_id(W w) { return w; }
Q q_ext(Q, int, Q);
F use(Q q, A a, N n) { return q_ext(q, 1, q_id(a_id(a).x[0] == 0 ? n.q : q)).x; }
v4 use_v(V v, W w) { return v_id(w_id(w).w[0]).v + v.v; }
}
