// GNU __builtin_assume_aligned (clang EmitBuiltinExpr ->
// emitAlignmentAssumption): the pointer is returned unchanged and an
// `llvm.assume(i1 true) ["align"(ptr, i64 A[, i64 off])]` operand bundle
// records the alignment; the optional offset is converted to size_t.
void *aa(void *p) { return __builtin_assume_aligned(p, 16); }
void *aa2(const int *p, int off) { return __builtin_assume_aligned(p, 32, off); }
template <int A> void *aa3(void *p) { return __builtin_assume_aligned(p, A); }
void *aa4(void *p) { return aa3<64>(p); }
