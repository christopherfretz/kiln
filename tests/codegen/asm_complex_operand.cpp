// WP-1288: a `_Complex` inline-asm output is indirect (`=*r`): clang
// `EmitAsmStmt` returns an output by value only for a scalar or aggregate
// evaluation kind. A `_Complex` input (or the input half of `+r`) is an
// integer of its size when that is at most 64 bits and a power of two,
// else it goes by memory (`*`, clang `EmitAsmInputLValue`). kiln returned
// `{ i64, i64 }` by value as an `i128`, which crashed `llc`
// (gcc.c-torture compile/20000804-1.c).

void out_big(_Complex long long *p) {
    _Complex long long v;
    asm("" : "=r"(v));
    *p = v;
}
void out_small(_Complex float *p) {
    _Complex float v;
    asm("" : "=r"(v));
    *p = v;
}
void in_small(_Complex float f) { asm volatile("" ::"r"(f)); }
void in_big(_Complex double d) { asm volatile("" ::"r"(d)); }
void in_mem(_Complex double d) { asm volatile("" ::"m"(d)); }
void inout_small(_Complex int *p) {
    _Complex int v = *p;
    asm("" : "+r"(v));
    *p = v;
}
