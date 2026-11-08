//@ verify
//@ args: --target=aarch64-unknown-linux-gnu
// GCC mode on AArch64 (clang `getRealTypeByWidth`): there is no 96-bit
// floating-point type and no `__float128`, so XF, XC, KF and KC are
// unsupported (TF and TC are the IEEE quad `long double`).
typedef float a __attribute__((mode(XF))); // expected-error {{unsupported machine mode 'XF'}}
typedef _Complex float b __attribute__((mode(XC))); // expected-error {{unsupported machine mode 'XC'}}
typedef float c __attribute__((mode(KF))); // expected-error {{unsupported machine mode 'KF'}}
typedef _Complex float d __attribute__((mode(KC))); // expected-error {{unsupported machine mode 'KC'}}
typedef _Complex float e __attribute__((mode(TC)));
static_assert(sizeof(e) == 32);
static_assert(sizeof(a) == sizeof(float));
