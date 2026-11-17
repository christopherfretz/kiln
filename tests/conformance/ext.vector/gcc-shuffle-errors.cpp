// GCC's __builtin_shuffle: two or three arguments; the operands vectors
// of the same type; the mask an integer vector with as many lanes as the
// operands, each of the same size (a floating, shorter, longer or
// narrower-laned mask is rejected); also when the operands are dependent,
// at instantiation. Errors in clang's style; GCC 16.1 rejects the same
// calls with its own wording. WP-1548.
//@ std: c++20
//@ expect: compile-fail
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_shuffle
//@ strict-diags
//@ diag: 31:28 error "too few arguments to function call, expected at least 2, have 1"
//@ diag: 32:37 error "too many arguments to function call, expected at most 3, have 4"
//@ diag: 33:27 error "first argument to '__builtin_shuffle' must be a vector"
//@ diag: 34:30 error "first two arguments to '__builtin_shuffle' must be vectors"
//@ diag: 35:27 error "first two arguments to '__builtin_shuffle' must have the same type"
//@ diag: 36:30 error "mask argument to '__builtin_shuffle' must be a vector of integers with as many elements as 'V' (vector of 4 'int' values), each of the same size ('F' (vector of 4 'float' values) invalid)"
//@ diag: 37:30 error "mask argument to '__builtin_shuffle' must be a vector of integers with as many elements as 'V' (vector of 4 'int' values), each of the same size ('L' (vector of 2 'long long' values) invalid)"
//@ diag: 38:30 error "mask argument to '__builtin_shuffle' must be a vector of integers with as many elements as 'V' (vector of 4 'int' values), each of the same size ('V8' (vector of 8 'int' values) invalid)"
//@ diag: 39:30 error "mask argument to '__builtin_shuffle' must be a vector of integers with as many elements as 'V' (vector of 4 'int' values), each of the same size ('S' (vector of 8 'short' values) invalid)"
//@ diag: 40:33 error "mask argument to '__builtin_shuffle' must be a vector of integers with as many elements as 'V' (vector of 4 'int' values), each of the same size ('F' (vector of 4 'float' values) invalid)"
//@ diag: 43:74 error "mask argument to '__builtin_shuffle' must be a vector of integers with as many elements as '__attribute__((__vector_size__(4 * sizeof(float)))) float' (vector of 4 'float' values), each of the same size ('__attribute__((__vector_size__(2 * sizeof(long long)))) long long' (vector of 2 'long long' values) invalid)"
//@ diag: 46:18 note "in instantiation of function template specialization 'sh<__attribute__((__vector_size__(4 * sizeof(float)))) float, __attribute__((__vector_size__(2 * sizeof(long long)))) long long>' requested here"
typedef int V __attribute__((vector_size(16)));
typedef short S __attribute__((vector_size(16)));
typedef long long L __attribute__((vector_size(16)));
typedef float F __attribute__((vector_size(16)));
typedef int V8 __attribute__((vector_size(32)));
V a, b; S s; L l; F f; V8 v8; int i;
void g() {
  (void)__builtin_shuffle(a);
  (void)__builtin_shuffle(a, b, a, a);
  (void)__builtin_shuffle(i, a);
  (void)__builtin_shuffle(a, i, a);
  (void)__builtin_shuffle(a, f, a);
  (void)__builtin_shuffle(a, f);
  (void)__builtin_shuffle(a, l);
  (void)__builtin_shuffle(a, v8);
  (void)__builtin_shuffle(a, s);
  (void)__builtin_shuffle(a, b, f);
  (void)__builtin_shuffle(f, b); // a float vector with an int mask: valid
}
template <class T, class M> T sh(T x, M m) { return __builtin_shuffle(x, m); }
template <class T, class M> T sh2(T x, T y, M m) { return __builtin_shuffle(x, y, m); }
V ok() { return sh(a, b) + sh2(a, b, b); }
F bad() { return sh(f, l); }
