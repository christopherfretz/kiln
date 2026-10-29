// [ptr.launder]/2, [c.math.fpclass], [bit.cast]/1: sema rejects a void or
// function pointer to __builtin_launder, non-floating operands to the
// ordered comparisons, and a size mismatch in __builtin_bit_cast.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:27 error "ordered compare requires two args of floating point type ('int' and 'int')"
//@ diag: 15:27 error "void pointer argument to '__builtin_launder' is not allowed"
//@ diag: 17:10 error "function pointer argument to '__builtin_launder' is not allowed"
//@ diag: 18:9 error "non-pointer argument to '__builtin_launder' is not allowed"
//@ diag: 19:21 error "__builtin_bit_cast source size does not equal destination size (8 vs 4)"
//@ diag: 20:34 error "1st argument must be an unsigned integer (was 'int')"
//@ diag: 21:24 error "1st argument must be an unsigned integer (was 'double')"
bool g = __builtin_isless(1, 2);
bool g2 = __builtin_isunordered(1.0, 2);
void *h(void *p) { return __builtin_launder(p); }
int fn();
auto i = __builtin_launder(fn);
int j = __builtin_launder(0);
constexpr float k = __builtin_bit_cast(float, 1.0);
unsigned l = __builtin_popcountg(-1);
int m = __builtin_clzg(1.0);
