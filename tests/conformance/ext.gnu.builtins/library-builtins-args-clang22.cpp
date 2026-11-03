// Argument checks of the GNU library builtins (clang 19 SemaChecking):
// __builtin_object_size's type is a constant in [0, 3]
// (BuiltinConstantArgRange); __builtin_verbose_trap's arguments are
// constant strings without '$' (checkBuiltinVerboseTrap);
// __builtin_assume_aligned takes 2 or 3 arguments, a pointer, a constant
// power-of-2 alignment (warned above 2^32) and a size_t offset
// (BuiltinAssumeAligned); __builtin_shufflevector takes two vectors of the
// same type and constant indices below twice the element count, and gives
// a vector_size vector when the index count differs (BuiltinShuffleVector).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 34:42 error "argument value 4 is outside the valid range [0, 3]"
//@ diag: 34:72 error "argument to '__builtin_object_size' must be a constant integer"
//@ diag: 34:102 error "argument value -1 is outside the valid range [0, 3]"
//@ diag: 35:35 error "argument to __builtin_verbose_trap must not contain $"
//@ diag: 35:70 error "argument to __builtin_verbose_trap must be a pointer to a constant string"
//@ diag: 36:33 error "requested alignment is not a power of 2"
//@ diag: 37:34 error "argument to '__builtin_assume_aligned' must be a constant integer"
//@ diag: 38:26 error "non-pointer argument to '__builtin_assume_aligned' is not allowed"
//@ diag: 39:53 error "too few arguments to function call, expected 2, have 1"
//@ diag: 40:27 warning "requested alignment must be 4294967296 bytes or smaller; maximum alignment assumed"
//@ diag: 41:62 error "too many arguments to function call, expected at most 3, have 4"
//@ diag: 42:33 error "first two arguments to '__builtin_shufflevector' must have the same type"
//@ diag: 42:73 error "index for __builtin_shufflevector must be a constant integer"
//@ diag: 42:113 error "index for __builtin_shufflevector must be less than the total number of vector elements"
//@ diag: 43:34 error "too few arguments to function call, expected at least 2, have 1"
//@ diag: 43:43 error "first two arguments to '__builtin_shufflevector' must be vectors"
//@ diag: 44:21 error "cannot initialize return object of type 'v4' (vector of 4 'int' values) with an rvalue of type 'int __attribute__((ext_vector_type(5)))' (vector of 5 'int' values)"
//@ requires: clang>=22
// clang 22 spelling of library-builtins-args.cpp (WP-394).
typedef int v4 __attribute__((ext_vector_type(4)));
typedef float f4 __attribute__((ext_vector_type(4)));
const char *m = "x";
unsigned long a(char *p, int i) { return __builtin_object_size(p, 4) + __builtin_object_size(p, i) + __builtin_dynamic_object_size(p, -1); }
void b() { __builtin_verbose_trap("a$", "m"); __builtin_verbose_trap(m, "m"); }
void *c(int *p, int n) { return __builtin_assume_aligned(p, 3); }
void *c2(int *p, int n) { return __builtin_assume_aligned(p, n); }
void *c3(int x) { return __builtin_assume_aligned(x, 4); }
void *c4(int *p) { return __builtin_assume_aligned(p); }
void *c5(int *p) { return __builtin_assume_aligned(p, 1LL << 33); }
void *c6(int *p) { return __builtin_assume_aligned(p, 4, 1, 2); }
v4 d(v4 x, f4 y, int n) { (void)__builtin_shufflevector(x, y, 0); (void)__builtin_shufflevector(x, x, n); (void)__builtin_shufflevector(x, x, 9);
  (void)__builtin_shufflevector(x); (void)__builtin_shufflevector(1, 2, 0); return __builtin_shufflevector(x, x, 0,1,2,3); }
v4 e(v4 x) { return __builtin_shufflevector(x, x, 0, 1, 2, 3, 4); }
