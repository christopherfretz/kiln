// [ptr.launder]/2, [c.math.fpclass], [bit.cast]/1: sema rejects a void or
// function pointer to __builtin_launder, non-floating operands to the
// ordered comparisons, and a size mismatch in __builtin_bit_cast.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:27 error "ordered compare requires two args of floating point type ('int' and 'int')"
//@ diag: 17:27 error "void pointer argument to '__builtin_launder' is not allowed"
//@ diag: 19:10 error "function pointer argument to '__builtin_launder' is not allowed"
//@ diag: 20:9 error "non-pointer argument to '__builtin_launder' is not allowed"
//@ diag: 21:21 error "size of '__builtin_bit_cast' source type 'double' does not match destination type 'float' (8 vs 4 bytes)"
//@ diag: 22:34 error "1st argument must be a scalar unsigned integer type (was 'int')"
//@ diag: 23:24 error "1st argument must be a scalar unsigned integer type (was 'double')"
//@ requires: clang>=22
// clang 22 spelling of builtins-sema-errors.cpp (WP-377).
bool g = __builtin_isless(1, 2);
bool g2 = __builtin_isunordered(1.0, 2);
void *h(void *p) { return __builtin_launder(p); }
int fn();
auto i = __builtin_launder(fn);
int j = __builtin_launder(0);
constexpr float k = __builtin_bit_cast(float, 1.0);
unsigned l = __builtin_popcountg(-1);
int m = __builtin_clzg(1.0);
