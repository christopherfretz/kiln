// [ACLE] Calls passing and returning fixed-length SVE types (ARM ACLE §3.7.3.3,
// clang AttrDocs "arm_sve_vector_bits"; the mangling is
// `9__SVE_VLSI<sizeless type>Lj<N>EE`). In memory a fixed-length type is
// `<M x T>`; a parameter or return is its scalable register type,
// converted with `llvm.vector.insert`/`extract` (a predicate through
// `<vscale x 2 x i8>`). spec/landed/codegen/WP-810.md (WP-810).
// Host-restricted (no pinned `--target`): on an x86_64 host clang has no
// AArch64 sysroot and leaves `+outline-atomics` out of the features.
//@ requires: clang>=22
//@ requires: target-aarch64
//@ expect: ir
//@ std: c++20
//@ args: -march=armv8-a+sve -msve-vector-bits=256
typedef __SVInt32_t svint32_t; typedef __SVBool_t svbool_t;
typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(256)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(256)));

fi32 id(fi32);
fb idb(fb);
svint32_t ids(svint32_t);
int use(fi32 a, fb p) {
  fi32 r = id(a);
  fb q = idb(p);
  svint32_t s = r;
  fi32 t = s;
  svbool_t sp = q;
  fb tq = sp;
  fi32 u = ids(t);
  return t[0] + tq[0] + u[1];
}
void q(fi32, svint32_t) {}
void pass(fi32 a, svint32_t s) { q(a, a); q(s, s); }
