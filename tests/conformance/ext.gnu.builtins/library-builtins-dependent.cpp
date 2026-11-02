// The argument checks of __builtin_shufflevector,
// __builtin_assume_aligned and __builtin_object_size wait for a
// value-dependent argument and run at instantiation ([temp.res]/6;
// clang TransformShuffleVectorExpr re-runs BuiltinShuffleVector).
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:38 error "index for __builtin_shufflevector must be less than the total number of vector elements"
//@ diag: 18:25 note "in instantiation of function template specialization 't<9>' requested here"
//@ diag: 16:44 error "requested alignment is not a power of 2"
//@ diag: 18:43 note "in instantiation of function template specialization 'u<6>' requested here"
//@ diag: 17:52 error "argument value 4 is outside the valid range [0, 3]"
//@ diag: 18:61 note "in instantiation of function template specialization 'o<4>' requested here"
typedef int v4 __attribute__((ext_vector_type(4)));
template <int N> v4 t(v4 x) { return __builtin_shufflevector(x, x, N, 0, 0, 0); }
template <int A> void *u(void *p) { return __builtin_assume_aligned(p, A); }
template <int A> unsigned long o(void *p) { return __builtin_object_size(p, A); }
void w(v4 x, void *p) { t<9>(x); t<7>(x); u<6>(p); u<8>(p); o<4>(p); o<1>(p); }
