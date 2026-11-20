// [AAPCS64 §5.4.2] Pure scalable types by value (clang 20+
// `passAsPureScalableType`, `coerceAndExpandPureScalableAggregate`): a
// class or array made only of fixed-length SVE vectors and predicates, at
// most 12 of them, is passed as one scalable register value per part when
// it is a named argument and all its Z and P registers are still free
// (counting the earlier SIMD/FP and predicate arguments), else by
// reference; a return value is one named argument with all registers
// free, several parts in a packed struct, more than 8 vectors in memory.
// The parts are read from and written to the object viewed as the
// flattened struct of its memory type, padding skipped; a call result is
// stored still scalable. A variadic argument always goes by reference and
// `va_arg` reads it so. Such a class is no homogeneous aggregate; a
// thunk forwards it as its target takes it.
// IR only: the test host has no SVE (WP-1739).
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
//@ std: c++20
//@ args: -march=armv8-a+sve -msve-vector-bits=256
typedef __SVInt32_t svint32_t; typedef __SVBool_t svbool_t;
typedef __SVFloat32_t svfloat32_t; typedef __SVFloat64_t svfloat64_t;
typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(256)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(256)));
typedef svfloat32_t ff32 __attribute__((arm_sve_vector_bits(256)));
typedef svfloat64_t ff64 __attribute__((arm_sve_vector_bits(256)));
struct A { fi32 v; };
struct D { fi32 v[2]; fb p; };
struct E { svint32_t dummy() const; ff64 x; fi32 y; };
struct P { fb p; fi32 v; };
struct Base { fi32 a; };
struct Der : Base { fb q[2]; };
struct Nine { fi32 v[9]; };
struct Thirteen { fi32 v[13]; };
struct Four { fi32 v[4]; };
struct FP { ff32 f; };
struct Empty {};
struct WithEmpty { Empty e; fi32 v; };
A ra();
void a(A);
void d(D);
void e(int, E);
void a_def(A x) { a(x); }
D d_def(D x) { d(x); return x; }
int call(A x, D y, E z) { a(x); d(y); e(1, z); return ra().v[0]; }
P rp(P x) { return x; }
Der rd(Der x) { return x; }
Nine rn(Nine x) { return x; }
Thirteen rt(Thirteen x) { return x; }
FP rf(FP x) { return x; }
void many(Four, Four, Four);
void manyf(double, double, double, double, double, double, double, Four, P);
void preds(fb, fb, fb, P, P);
void var(int, ...);
void we(WithEmpty);
void calls(P p, Four f, fb b, FP g, WithEmpty w) {
  many(f, f, f);
  manyf(1, 2, 3, 4, 5, 6, 7, f, p);
  preds(b, b, b, p, p);
  var(1, p, f);
  we(w);
  P q = rp(p);
  Nine n = rn(Nine{});
  (void)q;
  (void)n;
  rf(g);
}
D get(int n, ...) {
  __builtin_va_list ap;
  __builtin_va_start(ap, n);
  D r = __builtin_va_arg(ap, D);
  __builtin_va_end(ap);
  return r;
}
// A this-adjusting thunk takes and returns the parts as its target does.
struct L { virtual D f(D); };
struct R { virtual D f(D); int r; };
struct M : L, R { D f(D x) override { return x; } };
M m;
