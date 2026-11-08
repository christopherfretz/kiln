//@ verify
// GCC mode (clang `handleModeAttr`): an unknown mode name, a mode whose
// class (integer or floating-point) differs from the declared type's, a
// type that is neither, and a width the target has no type for are
// errors. An unknown or unsupported mode leaves the declared type
// unchanged; a mismatched one, as in clang, still installs the mode's type.
typedef int a __attribute__((mode(QQ))); // expected-error {{unknown machine mode 'QQ'}}
typedef float b __attribute__((mode(SI))); // expected-error {{type of machine mode does not match type of base type}}
typedef int c __attribute__((mode(DF))); // expected-error {{type of machine mode does not match type of base type}}
struct S {};
typedef S d __attribute__((mode(SI))); // expected-error {{mode attribute only supported for integer and floating-point types}}
typedef int *e __attribute__((mode(DI))); // expected-error {{mode attribute only supported for integer and floating-point types}}
typedef int f __attribute__((mode(XI))); // expected-error {{unsupported machine mode 'XI'}}
// Complex modes: the declared type must be a complex floating-point type
// (a complex integer type is not primitive), and an integer mode does not
// apply to one; `QC` (8 bits) has no floating-point type; `CQI` (GCC's
// complex integer mode) is not a mode clang knows.
typedef int g __attribute__((mode(SC))); // expected-error {{type of machine mode does not match type of base type}}
typedef float h __attribute__((mode(DC))); // expected-error {{type of machine mode does not match type of base type}}
typedef _Complex float i __attribute__((mode(SI))); // expected-error {{type of machine mode does not match type of base type}}
typedef _Complex int j __attribute__((mode(SC))); // expected-error {{mode attribute only supported for integer and floating-point types}}
typedef _Complex float k __attribute__((mode(QC))); // expected-error {{unsupported machine mode 'QC'}}
typedef _Complex int l __attribute__((mode(CQI))); // expected-error {{unknown machine mode 'CQI'}}
typedef int t __attribute__((mode(libgcc_cmp_return))); // expected-error {{unknown machine mode 'libgcc_cmp_return'}}
typedef int u __attribute__((mode(libgcc_shift_count))); // expected-error {{unknown machine mode 'libgcc_shift_count'}}
static_assert(sizeof(a) == sizeof(int));
static_assert(sizeof(b) == sizeof(float));
