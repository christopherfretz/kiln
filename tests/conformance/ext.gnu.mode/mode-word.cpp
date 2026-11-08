//@ expect: compile-ok
//@ strict-diags
// GCC mode word spellings (clang `parseModeAttrArg`): `byte` is the
// character width, `word` the register width, `pointer` the pointer width
// and `unwind_word` the unwinder word width: 8, 64, 64 and 64 bits on
// AArch64 and x86-64. Signedness is the declared type's. (`libgcc_cmp_return`
// and `libgcc_shift_count` are not modes clang knows: mode-errors.cpp.)
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };
typedef int b __attribute__((mode(byte)));
typedef unsigned ub __attribute__((__mode__(__byte__)));
typedef int w __attribute__((mode(word)));
typedef unsigned uw __attribute__((mode(__word__)));
typedef int p __attribute__((mode(pointer)));
typedef unsigned uu __attribute__((mode(unwind_word)));
static_assert(same<b, signed char>::value);
static_assert(same<ub, unsigned char>::value);
static_assert(same<w, long>::value);
static_assert(same<uw, unsigned long>::value);
static_assert(same<p, long>::value);
static_assert(same<uu, unsigned long>::value);
