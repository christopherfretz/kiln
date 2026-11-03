// [ACLE] `c ? t : f` with an SVE vector condition whose element is
// integral (`__SVBool_t` included) selects element-wise (clang
// `CheckSizelessVectorConditionalTypes`): two SVE arms must have the same
// type, which is the result; one SVE arm is converted as for a binary
// operator; two scalar arms get their common type, splatted to the SVE
// vector of the condition's element count; the result must have the
// condition's element count and element size. A floating condition is
// contextually converted to `bool`, which fails. spec/sema.md §5.8.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu -march=armv8-a+sve -ferror-limit=0
//@ strict-diags
//@ diag: 35:15 error "vector operands to the vector conditional must be the same type ('i32' (aka '__SVInt32_t') and 'u32' (aka '__SVUint32_t'))"
//@ diag: 40:15 error "vector condition type 'i32' (aka '__SVInt32_t') and result type 'i64' (aka '__SVInt64_t') do not have the same number of elements"
//@ diag: 42:15 error "vector condition type 'i32' (aka '__SVInt32_t') and result type 'f64' (aka '__SVFloat64_t') do not have the same number of elements"
//@ diag: 43:14 error "value of type 'f32' (aka '__SVFloat32_t') is not contextually convertible to 'bool'"
//@ diag: 45:16 error "vector condition type 'b' (aka '__SVBool_t') and result type 'i32' (aka '__SVInt32_t') do not have the same number of elements"
//@ diag: 46:16 error "vector condition type 'i32' (aka '__SVInt32_t') and result type '__clang_svint64x2_t' do not have the same number of elements"
//@ diag: 48:16 error "vector condition type 'i32' (aka '__SVInt32_t') and result type '__clang_svfloat64x2_t' do not have the same number of elements"
//@ diag: 49:16 error "enumeration type 'E' is not allowed in a vector conditional"
//@ diag: 50:16 error "cannot convert between scalar type 'E' and vector type 'i32' (aka '__SVInt32_t') as implicit conversion would cause truncation"
//@ diag: 51:16 error "vector condition type 'i32' (aka '__SVInt32_t') and result type 'i16' (aka '__SVInt16_t') do not have the same number of elements"
//@ diag: 52:16 error "cannot convert between scalar type 'double' and vector type 'i32' (aka '__SVInt32_t') as implicit conversion would cause truncation"
//@ diag: 59:45 error "cannot convert between scalar type 'double' and vector type '__SVInt32_t' as implicit conversion would cause truncation"
//@ diag: 61:24 note "in instantiation of function template specialization 'bad<__SVInt32_t>' requested here"

typedef __SVInt32_t i32; typedef __SVUint32_t u32; typedef __SVInt8_t i8;
typedef __SVFloat32_t f32; typedef __SVBool_t b; typedef __SVInt64_t i64;
typedef __SVFloat64_t f64; typedef __SVInt16_t i16;
enum E { X };

void f(i32 c, u32 u, i8 c8, f32 fl, b p, i64 l, f64 d, i16 h, int s, float x,
       long ll, E e, char ch, bool bb) {
  i32 a = c ? c : c;
  u32 r1 = c ? u : u;
  auto r2 = c ? c : u;
  i32 r3 = c ? 1 : 2;
  f32 r4 = c ? s : x;
  i32 r5 = c ? c : 1;
  i32 r6 = c ? 1L : c;
  auto r7 = c ? l : l;
  f32 r8 = c ? fl : fl;
  auto r9 = c ? d : d;
  auto r10 = fl ? c : c;
  i8 r11 = p ? c8 : c8;
  auto r12 = p ? c : c;
  auto r13 = c ? ll : ll;
  f32 r14 = c ? fl : x;
  auto r15 = c ? 1.0 : 2.0;
  auto r16 = c ? e : e;
  auto r17 = c ? c : e;
  auto r18 = c ? h : h;
  auto r19 = c ? c : 1.0;
  b r20 = p ? bb : bb;
  __SVUint8_t r21 = p ? ch : ch;
  u32 r22 = c ? 1u : 2;
}

template <class C, class T> auto sel(C c, T t, T f) { return c ? t : f; }
template <class C> auto bad(C c) { return c ? c : 1.0; }
i32 g(i32 c, i32 t, i32 f) { return sel(c, t, f); }
auto h(i32 c) { return bad(c); }
