// [ACLE] The AArch64 SVE sizeless builtin types (clang 19 oracle,
// AArch64SVEACLETypes.def): spec/types.md §5.1.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 55:19 error "invalid application of 'sizeof' to sizeless type '__SVInt8_t'"
//@ diag: 56:19 error "invalid application of 'alignof' to sizeless type 'svbool_t' (aka '__SVBool_t')"
//@ diag: 57:15 error "array has sizeless element type '__SVInt8_t'"
//@ diag: 58:23 error "field has sizeless type '__SVInt8_t'"
//@ diag: 59:33 error "field has sizeless type '__SVInt8_t'"
//@ diag: 60:15 note "in instantiation of template class 'W<__SVInt8_t>' requested here"
//@ diag: 61:12 error "non-local variable with sizeless type '__SVInt8_t'"
//@ diag: 62:19 error "non-local variable with sizeless type '__SVInt8_t'"
//@ diag: 63:31 error "non-local variable with sizeless type '__SVInt8_t'"
//@ diag: 64:33 error "non-local variable with sizeless type '__SVInt8_t'"
//@ diag: 64:61 error "non-local variable with sizeless type '__SVInt8_t'"
//@ diag: 70:14 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 71:20 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 72:9 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 69:6 error "SVE vector type '__SVInt8_t' cannot be used in a target without sve"
//@ diag: 74:13 error "SVE vector type 'svfloat32_t' (aka '__SVFloat32_t') cannot be used in a target without sve"
//@ diag: 74:13 error "SVE vector type 'svbool_t' (aka '__SVBool_t') cannot be used in a target without sve"
//@ diag: 74:13 error "SVE vector type 'svfloat32_t' (aka '__SVFloat32_t') cannot be used in a target without sve"
// The ACLE types are implicit typedefs of builtin types: usable in
// declarations, typedefs, templates and traits.
typedef __SVFloat32_t svfloat32_t;
typedef __SVBool_t svbool_t;
svfloat32_t vf(svfloat32_t, svbool_t);
void all(__SVInt8_t, __SVInt16_t, __SVInt32_t, __SVInt64_t, __SVUint8_t,
         __SVUint16_t, __SVUint32_t, __SVUint64_t, __SVFloat16_t,
         __SVFloat32_t, __SVFloat64_t, __SVBfloat16_t, __SVBool_t,
         __SVCount_t);
template <class T> struct S { static constexpr bool v = false; };
template <> struct S<__SVCount_t> { static constexpr bool v = true; };
static_assert(S<__SVCount_t>::v && !S<__SVBool_t>::v);
static_assert(__is_same(svfloat32_t, __SVFloat32_t));
static_assert(!__is_same(__SVInt8_t, __SVUint8_t));
template <class T> T *ptr_to();
__SVUint16_t *pu = ptr_to<__SVUint16_t>();

// Traits (clang 19): object, trivial and POD, but neither scalar nor
// fundamental nor compound, and not standard-layout or literal.
using V = __SVInt8_t;
static_assert(__is_object(V) && __is_trivially_copyable(V) && __is_pod(V));
static_assert(__is_trivial(V) && __is_trivially_constructible(V));
static_assert(__is_constructible(V, V) && __is_trivially_assignable(V &, V));
static_assert(__is_trivially_destructible(V) && __is_nothrow_destructible(V));
static_assert(__is_convertible(V, V) && __is_complete_type(V));
static_assert(!__is_scalar(V) && !__is_arithmetic(V) && !__is_fundamental(V));
static_assert(!__is_compound(V) && !__is_class(V) && !__is_aggregate(V));
static_assert(!__is_standard_layout(V) && !__is_literal_type(V));
static_assert(!__is_integral(V) && !__is_floating_point(V) && !__is_empty(V));

// Sizeless: no size, no arrays or fields, no static storage.
unsigned long s = sizeof(__SVInt8_t);
unsigned long a = alignof(svbool_t);
__SVInt8_t arr[2];
struct M { __SVInt8_t m; };
template <class T> struct W { T t; };
W<__SVInt8_t> w;
__SVInt8_t gv;
extern __SVInt8_t ev;
struct SM { static __SVInt8_t sm; };
void stat() { static __SVInt8_t sl; thread_local __SVInt8_t tl; }

// Without the sve target feature (kiln has none), clang rejects every
// value of these types in a function, and every use of a function or
// parameter that has one.
void uses(__SVInt8_t *p, __SVInt8_t x) {
  __SVInt8_t l;
  __SVInt8_t *q = &x;
  (void)x;
}
auto pvf = &vf;
