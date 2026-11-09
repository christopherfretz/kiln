// [expr.log.and]/[expr.log.or] on GNU vector operands: clang evaluates
// both operands (no short-circuit), compares each element with zero and
// sign-extends the combined i1 vector to the operand's element width
// (WP-941: kiln emitted a branch on the vector condition).
typedef int v4i __attribute__((vector_size(16)));
typedef float v4f __attribute__((vector_size(16)));
v4i land(v4i a, v4i b) { return a && b; }
v4i lor(v4i a, v4i b) { return a || b; }
v4i flt(v4f a, v4f b) { return a && b; }
