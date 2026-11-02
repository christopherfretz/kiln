// __builtin_convertvector and __builtin_reduce_* errors (clang
// SemaConvertVectorExpr, SemaBuiltinReduceMath) (WP-156).
//@ expect: compile-fail
//@ diag: 18:9 error "first two arguments to __builtin_convertvector must have the same number of elements"
//@ diag: 19:9 error "first argument to __builtin_convertvector must be a vector"
//@ diag: 20:9 error "second argument to __builtin_convertvector must be of vector type"
//@ diag: 21:30 error "1st argument must be a vector of"
//@ diag: 22:30 error "(was 'v4f' (vector of 4 'float' values))"
//@ diag: 23:30 error "(was 'v4f' (vector of 4 'float' values))"
//@ diag: 24:33 error "too many arguments to function call, expected 1, have 2"
// clang 19 and 22 word these differently: the substrings above are common
// to both, and kiln follows its -fclang-compat level (WP-377).
typedef int v4i __attribute__((ext_vector_type(4)));
typedef int v2i __attribute__((ext_vector_type(2)));
typedef float v4f __attribute__((ext_vector_type(4)));

void f(v4i a, v4f c, int x) {
  (void)__builtin_convertvector(a, v2i);
  (void)__builtin_convertvector(x, v4i);
  (void)__builtin_convertvector(a, int);
  (void)__builtin_reduce_add(x);
  (void)__builtin_reduce_and(c);
  (void)__builtin_reduce_add(c);
  (void)__builtin_reduce_max(a, a);
}
