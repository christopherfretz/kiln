// [ACLE] Implicit conversions of a fixed-length SVE type (ARM ACLE
// §3.7.3.3, clang AttrDocs "arm_sve_vector_bits"): to and from its sizeless
// type and, as lax vector conversions, to and from GNU vectors and other
// fixed-length SVE types of the same size; a size mismatch or a predicate
// against a data vector is an error (clang 19 oracle wording).
// spec/types.md "Fixed-length SVE types" (WP-810).
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -msve-vector-bits=512
//@ strict-diags
//@ diag: 41:13 error "cannot initialize a variable of type 'svint32_t' (aka '__SVInt32_t') with an lvalue of type 'fb' (vector of 8 'unsigned char' values)"
//@ diag: 42:13 error "cannot initialize a variable of type 'svint32_t' (aka '__SVInt32_t') with an lvalue of type 'gv32' (vector of 8 'int' values)"
//@ diag: 43:8 error "cannot initialize a variable of type 'fi32' (vector of 16 'int' values) with an lvalue of type 'gv32' (vector of 8 'int' values)"
typedef __SVInt32_t svint32_t; typedef __SVUint32_t svuint32_t; typedef __SVBool_t svbool_t;
typedef __SVFloat64_t svfloat64_t; typedef __SVCount_t svcount_t;
typedef svint32_t fi32 __attribute__((arm_sve_vector_bits(512)));
typedef svuint32_t fu32 __attribute__((arm_sve_vector_bits(512)));
typedef svfloat64_t ff64 __attribute__((arm_sve_vector_bits(512)));
typedef svbool_t fb __attribute__((arm_sve_vector_bits(512)));
typedef int gv __attribute__((vector_size(64)));
typedef double gvd __attribute__((vector_size(64)));
typedef unsigned char gb __attribute__((vector_size(8)));
typedef int gv32 __attribute__((vector_size(32)));
void f(fi32 a, fu32 b, ff64 c, fb d, gv e, gvd g, gb h, svint32_t s, svuint32_t u, svfloat64_t sf, svbool_t sb, gv32 x) {
  fi32 t1 = e;
  gv t2 = a;
  ff64 t3 = g;
  gvd t4 = c;
  fb t5 = h;
  gb t6 = d;
  fi32 t7 = b;
  svint32_t t8 = a;
  fi32 t9 = s;
  svuint32_t t10 = a;
  fu32 t11 = s;
  svbool_t t12 = d;
  fb t13 = sb;
  svint32_t t14 = e;
  gv t15 = s;
  svfloat64_t t16 = g;
  fi32 t17 = c;
  svint32_t t18 = d;
  svint32_t t19 = x;
  fi32 t20 = x;
  svfloat64_t t21 = a;
}
