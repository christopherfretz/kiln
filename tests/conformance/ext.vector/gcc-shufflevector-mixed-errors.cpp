// GCC's __builtin_shufflevector with vectors of different lengths: the
// element types must still be the same, and an index must be -1 or below
// the total lane count of both operands (Na + Nb: the boundary is
// tested). clang's diagnostics for each, as for operands of one type.
// GCC 16.1 rejects the same calls. WP-1548.
//@ std: c++20
//@ expect: compile-fail
//@ oracle: gcc
//@ skip(clang): clang 19/22 reject every call here, the valid ones too
//@ strict-diags
//@ diag: 26:9 error "index for __builtin_shufflevector must be less than the total number of vector elements"
//@ diag: 28:9 error "index for __builtin_shufflevector must be less than the total number of vector elements"
//@ diag: 29:9 error "first two arguments to '__builtin_shufflevector' must have the same type"
//@ diag: 30:9 error "first two arguments to '__builtin_shufflevector' must have the same type"
//@ diag: 31:9 error "first two arguments to '__builtin_shufflevector' must be vectors"
typedef int v4si __attribute__((vector_size(16)));
typedef int v8si __attribute__((vector_size(32)));
typedef long long v4di __attribute__((vector_size(32)));
typedef unsigned v8su __attribute__((vector_size(32)));
v4si a;
v8si b;
v4di l;
v8su u;
void f() {
  (void)__builtin_shufflevector(a, b, 0, 11);  // valid: b's last lane
  (void)__builtin_shufflevector(a, b, 0, 12);
  (void)__builtin_shufflevector(b, a, -1, 11, 8, 0); // valid: a's last lane
  (void)__builtin_shufflevector(a, b, 0, -2);
  (void)__builtin_shufflevector(a, l, 0, 1);
  (void)__builtin_shufflevector(a, u, 0, 1);
  (void)__builtin_shufflevector(a, 1, 0, 1);
}
