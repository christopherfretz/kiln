//@ verify
// GCC mode (clang `handleModeAttr`): an unknown mode name, a mode whose
// class (integer or floating-point) differs from the declared type's, a
// type that is neither, and a width the target has no type for are
// errors; the declared type is left unchanged.
typedef int a __attribute__((mode(QQ))); // expected-error {{unknown machine mode 'QQ'}}
typedef float b __attribute__((mode(SI))); // expected-error {{type of machine mode does not match type of base type}}
typedef int c __attribute__((mode(DF))); // expected-error {{type of machine mode does not match type of base type}}
struct S {};
typedef S d __attribute__((mode(SI))); // expected-error {{mode attribute only supported for integer and floating-point types}}
typedef int *e __attribute__((mode(DI))); // expected-error {{mode attribute only supported for integer and floating-point types}}
typedef int f __attribute__((mode(XI))); // expected-error {{unsupported machine mode 'XI'}}
static_assert(sizeof(a) == sizeof(int));
static_assert(sizeof(b) == sizeof(float));
