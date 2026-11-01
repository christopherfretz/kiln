// Vector operand diagnostics (clang CheckVectorOperands, checkVectorShift,
// CreateBuiltinArraySubscriptExpr): mixed vector types, a scalar that
// does not fit the element type, shifts of float vectors, the address of
// a vector element, a non-const reference to one, and casts between a
// vector and a vector or integer of a different size, or between an integer and an ext
// vector (TryReinterpretCast, areLaxCompatibleVectorTypes)
// (WP-156).
//@ expect: compile-fail
//@ diag: 30:12 error "cannot convert between vector values of different size ('v4i' (vector of 4 'int' values) and 'v2i' (vector of 2 'int' values))"
//@ diag: 32:12 error "used type 'v4f' (vector of 4 'float' values) where integer is required"
//@ diag: 34:9 error "address of vector element requested"
//@ diag: 35:8 error "non-const reference cannot bind to vector element"
//@ diag: 36:12 error "invalid operands to binary expression ('v4i' (vector of 4 'int' values) and 'v4f' (vector of 4 'float' values))"
//@ diag: 37:12 error "cannot convert between vector type 'v2i' (vector of 2 'int' values) and vector type 'g4i' (vector of 4 'int' values) as implicit conversion would cause truncation"
//@ diag: 38:10 error "array subscript is not an integer"
//@ diag: 39:9 error "C-style cast from vector 'v4i' (vector of 4 'int' values) to vector 'v2i' (vector of 2 'int' values) of different size"
//@ diag: 40:9 error "C-style cast from vector 'v4i' (vector of 4 'int' values) to scalar 'short' of different size"
//@ diag: 42:9 error "reinterpret_cast from vector 'v4f' (vector of 4 'float' values) to vector 'v2i' (vector of 2 'int' values) of different size"
//@ diag: 43:9 error "C-style cast from vector 'v2i' (vector of 2 'int' values) to scalar 'long' of different size"
//@ diag: 47:9 error "C-style cast from vector 'v8c' (vector of 8 'char' values) to scalar 'int' of different size"
//@ diag: 48:9 error "reinterpret_cast from scalar 'long' to vector 'v2i' (vector of 2 'int' values) of different size"
typedef int v4i __attribute__((ext_vector_type(4)));
typedef int v2i __attribute__((ext_vector_type(2)));
typedef float v4f __attribute__((ext_vector_type(4)));
typedef int g4i __attribute__((vector_size(16)));
typedef int g2i __attribute__((vector_size(8)));
typedef char v8c __attribute__((ext_vector_type(8)));

void f(v4i a, v2i b, v4f c, g4i g, long l, g2i h, v8c k) {
  (void)(a + b);
  (void)(a + c);
  (void)(c << 1);
  (void)(a < c);
  (void)&a[0];
  int& r = a[1];
  (void)(a % c);
  (void)(g + b);
  (void)a[0.5];
  (void)(v2i)a;
  (void)(short)a;
  (void)(v4i)l;
  (void)reinterpret_cast<v2i>(c);
  (void)(long)b;
  (void)(v4f)a;
  (void)reinterpret_cast<g4i>(c);
  (void)(long)h;
  (void)(int)k;
  (void)reinterpret_cast<v2i>(l);
  (void)(g2i)l;
  (void)(v2i)h;
  (void)(v8c)b;
}
