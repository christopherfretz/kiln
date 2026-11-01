// clang `getTypeInfo` for vector types: an ext_vector of bool packs one bit
// per element (width N bits, at least 8, rounded up to a power of two);
// other vectors are elem-size * N, aligned to their (power-of-two) size
// capped at the target's max vector alignment (WP-151, simd_utils.h:148).
typedef bool b1 __attribute__((ext_vector_type(1)));
typedef bool b3 __attribute__((ext_vector_type(3)));
typedef bool b8 __attribute__((ext_vector_type(8)));
typedef bool b12 __attribute__((ext_vector_type(12)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef bool b32 __attribute__((ext_vector_type(32)));
typedef bool b64 __attribute__((ext_vector_type(64)));
typedef int i3 __attribute__((ext_vector_type(3)));
typedef double d8 __attribute__((ext_vector_type(8)));
typedef short s4 __attribute__((vector_size(8)));
struct S {
  char c0; b1 v1; char c1; b3 v3; char c2; b8 v8; char c3; b12 v12;
  char c4; b16 v16; char c5; b32 v32; char c6; b64 v64;
};
struct T { char c0; i3 v; char c1; d8 w; char c2; s4 x; };
