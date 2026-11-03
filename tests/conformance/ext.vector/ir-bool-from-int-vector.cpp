// Clang ext vectors, [clang 20+] (clang ScalarExprEmitter::EmitScalarConversion,
// CK_IntegralToBoolean on a vector): the implicit integer ext vector ->
// `bool` ext vector conversion is a lane-wise `icmp ne` zero giving
// `<N x i1>`, stored on the packed `bool` vector layout (WP-450), in
// initialization, a return, assignment, a cast, a call argument and a
// template. clang 19 rejects the conversion. WP-453.
//@ expect: ir
//@ requires: clang>=22
//@ requires: llvm>=22
typedef int i4 __attribute__((ext_vector_type(4)));
typedef unsigned char uc4 __attribute__((ext_vector_type(4)));
typedef long l4 __attribute__((ext_vector_type(4)));
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef short s16 __attribute__((ext_vector_type(16)));
b4 f(i4 I) { b4 L = I < 4; return L; }
b4 g(uc4 U) { return U; }
b4 h(l4 L) { b4 r = {}; r = L; return r; }
b16 k(s16 S) { return static_cast<b16>(S); }
void use(b4);
void m(i4 I) { use(I); use((b4)(I == 0)); }
template <class T> b4 conv(T t) { return t; }
b4 n(i4 I) { return conv(I); }
bool lane(i4 I) { b4 x = I; return x[1]; }
