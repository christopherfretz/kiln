// ext_vector_type / vector_size attribute errors (clang
// HandleExtVectorTypeAttr / HandleVectorSizeAttr, BuildExtVectorType /
// BuildVectorType). In an instantiation kiln points at the size operand,
// clang at the attribute name (same line).
//@ expect: compile-fail
//@ diag: 18:22 error "'ext_vector_type' attribute only applies to typedefs"
//@ diag: 19:30 error "zero vector size"
//@ diag: 20:31 error "invalid vector element type 'int *'"
//@ diag: 21:31 error "ext_vector_type attribute requires an integer constant"
//@ diag: 22:31 error "vector size not an integral multiple of component size"
//@ diag: 23:32 error "invalid vector element type 'bool'"
//@ diag: 24:31 error "'ext_vector_type' attribute takes one argument"
//@ diag: 27 error "zero vector size"
//@ diag: 29:10 note "in instantiation of template type alias 'vec' requested here"
//@ requires: clang<22
// clang 19 spelling; diagnostics-clang22.cpp has clang 22's (WP-394).

int v __attribute__((ext_vector_type(4)));
typedef int z __attribute__((ext_vector_type(0)));
typedef int* p __attribute__((ext_vector_type(2)));
typedef int nm __attribute__((ext_vector_type(1.5)));
typedef int vs __attribute__((vector_size(6)));
typedef bool vb __attribute__((vector_size(4)));
typedef int nn __attribute__((ext_vector_type));

template <class T, int N>
using vec __attribute__((ext_vector_type(N))) = T;
template <int N> struct Z {};
Z<sizeof(vec<int, 0>)> zz;
