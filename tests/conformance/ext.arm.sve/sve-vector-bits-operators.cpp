// [ACLE] Operators on fixed-length SVE types (ARM ACLE §3.7.3.3, clang
// AttrDocs "arm_sve_vector_bits"): the GNU vector operators, subscripts and
// brace initialization; comparison and conditional result types; mixing
// with the sizeless type or GNU vectors (clang 19 oracle wording; the
// `show<>` errors print the result types). spec/types.md "Fixed-length
// SVE types" (WP-810).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -msve-vector-bits=512
//@ strict-diags
//@ diag: 43:15 warning "self-comparison always evaluates to true"
//@ diag: 43:40 error "implicit instantiation of undefined template 'show<__attribute__((__vector_size__(16 * sizeof(int)))) int>'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 44:39 error "implicit instantiation of undefined template 'show<__attribute__((__arm_sve_vector_bits__(16 * sizeof(int) * 8))) int>'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 45:15 warning "self-comparison always evaluates to true"
//@ diag: 45:40 error "implicit instantiation of undefined template 'show<__attribute__((__vector_size__(8 * sizeof(char)))) char>'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 46:39 error "implicit instantiation of undefined template 'show<__attribute__((__vector_size__(8 * sizeof(long long)))) long long>'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 47:43 error "implicit instantiation of undefined template 'show<__attribute__((__arm_sve_vector_bits__(16 * sizeof(int) * 8))) int>'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 48:15 error "cannot combine fixed-length and sizeless SVE vectors in expression, result is ambiguous ('fi32' (vector of 16 'int' values) and 'svint32_t' (aka '__SVInt32_t'))"
//@ diag: 49:15 error "cannot combine GNU and SVE vectors in expression, result is ambiguous ('gv' (vector of 16 'int' values) and 'fi32' (vector of 16 'int' values))"
//@ diag: 50:15 error "cannot combine GNU and SVE vectors in expression, result is ambiguous ('gv' (vector of 16 'int' values) and 'svint32_t' (aka '__SVInt32_t'))"
//@ diag: 51:13 error "invalid argument type 'fi32' (vector of 16 'int' values) to unary expression"
//@ diag: 52:40 error "implicit instantiation of undefined template 'show<unsigned char>'"
//@ diag: 37:26 note "template is declared here"
//@ diag: 53:17 error "cannot combine fixed-length and sizeless SVE vectors in expression, result is ambiguous ('svbool_t' (aka '__SVBool_t') and 'fb' (vector of 8 'unsigned char' values))"
//@ diag: 54:16 error "cannot convert between scalar type 'double' and vector type 'fi32' (vector of 16 'int' values) as implicit conversion would cause truncation"
typedef __SVInt32_t svint32_t; typedef __SVUint32_t svuint32_t; typedef __SVBool_t svbool_t;
typedef __SVFloat64_t svfloat64_t; typedef __SVCount_t svcount_t;
typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(512)));
typedef svuint32_t fu32 __attribute__((arm_sve_vector_bits(512)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(512)));
typedef svfloat64_t ff64 __attribute__((arm_sve_vector_bits(512)));
typedef int gv __attribute__((vector_size(64)));
template<class T> struct show;
void f(fi32 a, fu32 b, fb p, ff64 d, svint32_t s, gv g, svbool_t sp) {
  fi32 r1 = a + 1; r1 = a * a; r1 = -a; r1 = ~a; r1 = a << 2; r1 += 3;
  fb q = p & p; q = p | p; q = ~p; q = p + p;
  unsigned char c = p[0]; int i = a[1]; a[1] = 3;
  fi32 br = {1, 2, 3};
  auto x1 = a == a; show<decltype(x1)> s1;
  auto x2 = a + b; show<decltype(x2)> s2;
  auto x3 = p == p; show<decltype(x3)> s3;
  auto x4 = d < d; show<decltype(x4)> s4;
  auto x5 = a ? a : a; show<decltype(x5)> s5;
  auto x6 = a + s;
  auto x7 = g + a;
  auto x8 = g + s;
  auto x9 = !a;
  auto x10 = p[0]; show<decltype(x10)> s10;
  auto x11 = sp + p;
  auto x12 = a + 1.5;
  auto x13 = d + 1;
}
