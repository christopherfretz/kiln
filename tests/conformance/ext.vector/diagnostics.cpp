// ext_vector_type / vector_size attribute errors (clang
// HandleExtVectorTypeAttr / HandleVectorSizeAttr, BuildExtVectorType /
// BuildVectorType). In an instantiation kiln points at the size operand,
// clang at the attribute name (same line).
//@ expect: compile-fail
//@ diag: 16:22 error "'ext_vector_type' attribute only applies to typedefs"
//@ diag: 17:30 error "zero vector size"
//@ diag: 18:31 error "invalid vector element type 'int *'"
//@ diag: 19:31 error "ext_vector_type attribute requires an integer constant"
//@ diag: 20:31 error "vector size not an integral multiple of component size"
//@ diag: 21:32 error "invalid vector element type 'bool'"
//@ diag: 22:31 error "'ext_vector_type' attribute takes one argument"
//@ diag: 25 error "zero vector size"
//@ diag: 27:10 note "in instantiation of template type alias 'vec' requested here"

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
