//@ requires: clang>=22
//@ strict-diags
//@ args: -ffp-model=fast
//@ diag: 13:30 warning "complex division behavior [-Whigher-precision-for-complex-division]"
// WP-1795: clang 22's -ffp-model=fast selects promoted complex
// arithmetic, so a `long double` complex divisor (no wider type) warns
// once at the end of the translation unit, without the "additional
// uses" clause for a single use; a `/` mixing a `float` complex with a
// `long double` complex divides in `long double` after the usual
// arithmetic conversions.
using CF = _Complex float;
using CL = _Complex long double;
CL q1(CF a, CL b) { return a / b; }
CF q2(CF a, CF b) { return a / b; }
