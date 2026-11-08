//@ args: -g -Xclang -target-feature -Xclang +sve
// Debug information for the AArch64 SVE builtin types (WP-910): clang's
// `DW_TAG_array_type` with `DIFlagVector` over the element type, whose one
// subrange's upper bound is a `DW_OP_bregx 46` (VG) expression; the
// predicate counts bytes, `svcount_t` is a one-element array, and a tuple
// multiplies the element count. Each type is named by the implicit typedef
// of its spelling, filed in the main file without a line.
#ifdef __aarch64__
void locals() {
  __SVInt8_t a1; __SVInt16_t a2; __SVInt32_t a3; __SVInt64_t a4;
  __SVUint8_t b1; __SVUint16_t b2; __SVUint32_t b3; __SVUint64_t b4;
  __SVFloat16_t c1; __SVFloat32_t c2; __SVFloat64_t c3; __SVBfloat16_t c4;
  __SVBool_t d1; __SVCount_t d2;
}
void pointers(__SVInt32_t *p, __SVBool_t *q) {}
// The tuples are reached through pointers: clang 22 allocates a tuple
// local as a struct of vectors, clang 19 and kiln as one wide vector, a
// lowering difference outside debug information.
void tuples(__clang_svint8x2_t *e1, __clang_svuint16x3_t *e2,
            __clang_svfloat64x4_t *e3, __clang_svboolx2_t *e4) {}
#endif
