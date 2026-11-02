// GNU checked arithmetic builtins ([gnu] __builtin_*_overflow; clang
// EmitBuiltinExpr): the encompassing-width generic lowering, the
// mixed-sign and unsigned-to-signed multiplications, and the typed forms.
enum E { e0 = -1 };
enum class S : short { s0 };

bool f1(int a, int b, int *r) { return __builtin_add_overflow(a, b, r); }
bool f2(unsigned a, unsigned b, unsigned *r) { return __builtin_mul_overflow(a, b, r); }
bool f3(int a, unsigned b, int *r) { return __builtin_sub_overflow(a, b, r); }
bool f4(long long a, long long b, int *r) { return __builtin_add_overflow(a, b, r); }
bool f5(int a, unsigned b, long long *r) { return __builtin_mul_overflow(a, b, r); }
bool f6(unsigned a, unsigned b, int *r) { return __builtin_mul_overflow(a, b, r); }
bool f7(__int128 a, unsigned __int128 b, __int128 *r) { return __builtin_add_overflow(a, b, r); }
bool f8(long long a, unsigned b, unsigned *r) { return __builtin_mul_overflow(a, b, r); }
bool f9(short a, unsigned long long b, long long *r) { return __builtin_mul_overflow(a, b, r); }
bool f10(int a, int b, bool *r) { return __builtin_sub_overflow(a, b, r); }
bool f11(bool a, char b, volatile signed char *r) { return __builtin_add_overflow(a, b, r); }
bool f12(E a, unsigned char b, unsigned short *r) { return __builtin_mul_overflow(a, b, r); }
bool f13(int a, unsigned b, bool *r) { return __builtin_mul_overflow(a, b, r); }
bool f14(unsigned __int128 a, unsigned __int128 b, __int128 *r) { return __builtin_mul_overflow(a, b, r); }
bool f15(int a, int *r) { return __builtin_mul_overflow(a, 3u, r); }
bool f16(unsigned long a, long *r) { return __builtin_mul_overflow(-5, a, r); }

struct alignas(16) A { int x; };
bool f17(A &a, int b) { return __builtin_add_overflow(b, 1, &a.x); }

bool t1(int a, int b, int *r) { return __builtin_sadd_overflow(a, b, r); }
bool t2(long a, long b, long *r) { return __builtin_ssubl_overflow(a, b, r); }
bool t3(long long a, long long b, long long *r) { return __builtin_smulll_overflow(a, b, r); }
bool t4(unsigned a, unsigned b, unsigned *r) { return __builtin_uadd_overflow(a, b, r); }
bool t5(unsigned long a, unsigned long b, unsigned long *r) { return __builtin_usubl_overflow(a, b, r); }
bool t6(unsigned long long a, unsigned long long b, unsigned long long *r) {
  return __builtin_umulll_overflow(a, b, r);
}
bool t7(short a, char b, int *r) { return __builtin_smul_overflow(a, b, r); }

int g;
bool cst() { return __builtin_add_overflow(1, 2, &g); }
