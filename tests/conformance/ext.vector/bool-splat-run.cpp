// [WP-3698] Constant and dynamic `ext_vector_type` bool splats
// (`CastKind::BooleanToSignedIntegral`): `true` splats to -1 / all-ones
// (-1.0 for floating vectors) and `false` to 0 across signed, unsigned,
// floating, and 128-bit integer element types, and a dynamic `bool` global
// splat compiles and initializes properly at run time.
//@ expect: run
#include <cstdio>

typedef int V4i __attribute__((ext_vector_type(4)));
typedef signed char V4sc __attribute__((ext_vector_type(4)));
typedef char V4c __attribute__((ext_vector_type(4)));
typedef long V2l __attribute__((ext_vector_type(2)));
typedef unsigned int V4u __attribute__((ext_vector_type(4)));
typedef unsigned char V4uc __attribute__((ext_vector_type(4)));
typedef unsigned long V2ul __attribute__((ext_vector_type(2)));
typedef float V4f __attribute__((ext_vector_type(4)));
typedef __int128 V2i128 __attribute__((ext_vector_type(2)));

struct I4 { int x[4]; };
struct SC4 { signed char x[4]; };
struct L2 { long x[2]; };
struct U4 { unsigned int x[4]; };
struct UC4 { unsigned char x[4]; };
struct UL2 { unsigned long x[2]; };
struct F4 { float x[4]; };
struct I128_2 { __int128 x[2]; };

constexpr V4i cv_it = (V4i)true;
constexpr V4i cv_if = (V4i)false;
constexpr V4sc cv_sct = (V4sc)true;
constexpr V4sc cv_scf = (V4sc)false;
constexpr V4c cv_ct = (V4c)true;
constexpr V2l cv_lt = (V2l)true;
constexpr V2l cv_lf = (V2l)false;
constexpr V4u cv_ut = (V4u)true;
constexpr V4u cv_uf = (V4u)false;
constexpr V4uc cv_uct = (V4uc)true;
constexpr V4uc cv_ucf = (V4uc)false;
constexpr V2ul cv_ult = (V2ul)true;
constexpr V4f cv_ft = (V4f)true;
constexpr V4f cv_ff = (V4f)false;
constexpr V2i128 cv_i128t = (V2i128)true;
constexpr V2i128 cv_i128f = (V2i128)false;

constexpr V4i mk_i(bool b) { return (V4i)b; }

static_assert(__builtin_bit_cast(I4, cv_it).x[0] == -1 &&
              __builtin_bit_cast(I4, cv_it).x[3] == -1);
static_assert(__builtin_bit_cast(I4, cv_if).x[0] == 0 &&
              __builtin_bit_cast(I4, cv_if).x[3] == 0);
static_assert(__builtin_bit_cast(I4, mk_i(true)).x[2] == -1 &&
              __builtin_bit_cast(I4, mk_i(false)).x[2] == 0);
static_assert(__builtin_bit_cast(SC4, cv_sct).x[0] == -1 &&
              __builtin_bit_cast(SC4, cv_scf).x[3] == 0);
static_assert(__builtin_bit_cast(UC4, cv_ct).x[0] == 0xffu);
static_assert(__builtin_bit_cast(L2, cv_lt).x[0] == -1L &&
              __builtin_bit_cast(L2, cv_lf).x[1] == 0L);
static_assert(__builtin_bit_cast(U4, cv_ut).x[0] == ~0u &&
              __builtin_bit_cast(U4, cv_uf).x[3] == 0u);
static_assert(__builtin_bit_cast(UC4, cv_uct).x[0] == 0xffu &&
              __builtin_bit_cast(UC4, cv_ucf).x[3] == 0u);
static_assert(__builtin_bit_cast(UL2, cv_ult).x[0] == ~0ul);
static_assert(__builtin_bit_cast(F4, cv_ft).x[0] == -1.0f &&
              __builtin_bit_cast(F4, cv_ff).x[3] == 0.0f);
static_assert(__builtin_bit_cast(I128_2, cv_i128t).x[0] == -1 &&
              __builtin_bit_cast(I128_2, cv_i128t).x[1] == -1 &&
              __builtin_bit_cast(I128_2, cv_i128f).x[0] == 0);

// Dynamic global splats (the WP-3698 reproducer).
bool bb = true;
bool bf = false;
V4i dyn_t = (V4i)bb;
V4i dyn_f = (V4i)bf;

int main() {
  std::printf("ci: %d %d %d %d | %d %d %d %d\n",
              cv_it[0], cv_it[1], cv_it[2], cv_it[3],
              cv_if[0], cv_if[1], cv_if[2], cv_if[3]);
  std::printf("csc: %d %d | cuc: %u %u | cu: %u %u | cl: %ld %ld | cul: %lu\n",
              (int)cv_sct[0], (int)cv_scf[0],
              (unsigned)cv_uct[0], (unsigned)cv_ucf[0],
              cv_ut[0], cv_uf[0],
              cv_lt[0], cv_lf[0],
              cv_ult[0]);
  std::printf("cf: %.1f %.1f | ci128: %d %d\n",
              cv_ft[0], cv_ff[0],
              (int)cv_i128t[0], (int)cv_i128f[0]);
  std::printf("dyn: %d %d %d %d | %d %d %d %d\n",
              dyn_t[0], dyn_t[1], dyn_t[2], dyn_t[3],
              dyn_f[0], dyn_f[1], dyn_f[2], dyn_f[3]);
  bool ok = dyn_t[0] == -1 && dyn_t[3] == -1 &&
            dyn_f[0] == 0 && dyn_f[3] == 0 &&
            cv_it[0] == -1 && cv_if[0] == 0 &&
            cv_ut[0] == ~0u && cv_uct[0] == 0xffu &&
            cv_ft[0] == -1.0f && cv_i128t[0] == -1;
  return ok ? 0 : 1;
}
