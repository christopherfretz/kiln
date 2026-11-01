// [WP-156] AAPCS64 vector argument and return lowering (clang
// AArch64ABIInfo::isIllegalVectorType / coerceIllegalVector): illegal
// vectors go as i32, <2 x i32>, <4 x i32> or indirectly; vectors wider
// than 128 bits return through sret, reloaded in the epilogue.
// x86-64 (X86_64ABIInfo::classify): 8-32 bit vectors are INTEGER, 64-bit
// ones SSE (<1 x double> in memory), 128-bit SSE+SSEUP; illegal vectors in
// memory pass byval.
typedef bool b4 __attribute__((ext_vector_type(4)));
typedef bool b16 __attribute__((ext_vector_type(16)));
typedef int i4 __attribute__((ext_vector_type(4)));
typedef int i3 __attribute__((ext_vector_type(3)));
typedef char c4 __attribute__((ext_vector_type(4)));
typedef int i8v __attribute__((ext_vector_type(8)));
typedef long l1 __attribute__((ext_vector_type(1)));
typedef bool b64 __attribute__((ext_vector_type(64)));
typedef int i2 __attribute__((ext_vector_type(2)));
typedef char c3 __attribute__((ext_vector_type(3)));
typedef char c2 __attribute__((ext_vector_type(2)));
typedef double d1 __attribute__((ext_vector_type(1)));
typedef float f2 __attribute__((ext_vector_type(2)));
typedef float f3 __attribute__((ext_vector_type(3)));
typedef short s2 __attribute__((vector_size(4)));
typedef double d4 __attribute__((vector_size(32)));
b4 fb4(b4 x) { return x; }
b16 fb16(b16 x) { return x; }
i3 fi3(i3 x) { return x; }
c4 fc4(c4 x) { return x; }
i8v fi8(i8v x) { return x; }
l1 fl1(l1 x) { return x; }
i4 fi4(i4 x) { return x; }
void cp(const i4* p, const b4* q) {}
b64 fb64(b64 x) { return x; }
i2 fi2(i2 x) { return x; }
c3 fc3(c3 x) { return x; }
c2 fc2(c2 x) { return x; }
d1 fd1(d1 x) { return x; }
f2 ff2(f2 x) { return x; }
f3 ff3(f3 x) { return x; }
s2 fs2(s2 x) { return x; }
d4 fd4(d4 x) { return x; }
struct S { f2 a; f2 b; };
S fs(S s) { return s; }
int call(i8v a, b4 b, f2 c) { fi8(a); fb4(b); ff2(c); return 0; }

// Vector attributes among the decl-specifiers apply to the return and
// parameter types (WP-156).
__attribute__((vector_size(8))) short ret_g(__attribute__((vector_size(8))) short x) { return x + 1; }
__attribute__((vector_size(16))) int *ret_gp(__attribute__((vector_size(16))) int *p) { return p + 1; }
