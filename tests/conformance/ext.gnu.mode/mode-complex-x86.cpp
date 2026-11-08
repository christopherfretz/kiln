// GCC mode on x86-64 (clang `handleModeAttr`, `getRealTypeByWidth`): XF
// and XC are the x87 `long double` and its complex type; TF, KF and the
// complex TC, KC are `__float128` (x86-64's `long double` is not IEEE
// quad); glibc's <bits/floatn.h> `__cfloat128` is the TC typedef below.
//@ args: --target=x86_64-unknown-linux-gnu
//@ verify
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

typedef float xf __attribute__((mode(XF)));
typedef float tf __attribute__((mode(TF)));
typedef double kf __attribute__((mode(KF)));
typedef _Complex float xc __attribute__((mode(XC)));
typedef _Complex float __cfloat128 __attribute__((__mode__(__TC__)));
typedef _Complex double kc __attribute__((mode(KC)));
typedef int w __attribute__((mode(word)));
static_assert(same<xf, long double>::value);
static_assert(same<tf, __float128>::value);
static_assert(same<kf, __float128>::value);
static_assert(same<xc, _Complex long double>::value);
static_assert(same<__cfloat128, _Complex __float128>::value);
static_assert(same<kc, _Complex __float128>::value);
static_assert(same<w, long>::value);
static_assert(sizeof(__cfloat128) == 32 && alignof(__cfloat128) == 16);
static_assert(sizeof(xc) == 32 && alignof(xc) == 16);
static_assert(sizeof(xf) == 16 && alignof(tf) == 16);
typedef float v2xf __attribute__((mode(V2XF))); // expected-warning {{deprecated}}
static_assert(sizeof(v2xf) == 32);
