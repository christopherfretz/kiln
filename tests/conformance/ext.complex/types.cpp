// GNU _Complex types, conversions and operators under clang 19 (WP-211).
//@ expect: compile-ok
//@ diag: 9:1 warning "plain '_Complex' requires a type specifier; assuming '_Complex double'"
using C = _Complex double;
using CF = _Complex float;
using CL = _Complex long double;
using CI = _Complex int;
using CU = _Complex unsigned char;
_Complex plain;
static_assert(__is_same(decltype(plain), _Complex double));
static_assert(sizeof(C) == 2 * sizeof(double) && alignof(C) == alignof(double));
static_assert(sizeof(CF) == 8 && alignof(CF) == 4);
static_assert(sizeof(CL) == 2 * sizeof(long double));
static_assert(sizeof(CU) == 2 && alignof(CU) == 1);
// Usual arithmetic conversions: the wider element type; an integer side
// becomes floating.
static_assert(__is_same(decltype(CF() + 1.0), C));
static_assert(__is_same(decltype(CF() + 1), CF));
static_assert(__is_same(decltype(CI() + 1.0f), CF));
static_assert(__is_same(decltype(CI() + CF()), CF));
static_assert(__is_same(decltype(CI() + 1L), _Complex long));
// No integral promotion of complex integer elements (clang).
static_assert(__is_same(decltype(CU() + CU()), CU));
static_assert(__is_same(decltype(C() == 1), bool));
static_assert(__is_same(decltype(__real__ C()), double));
static_assert(__is_same(decltype(__imag__ CI()), int));
static_assert(__is_same(decltype(__real__ 1.0f), float));
static_assert(__is_same(decltype(~C()), C));
// __real__/__imag__ of an lvalue are lvalues; ++ steps the real part.
void parts(C& z, CI& i) {
  __real__ z = 1;
  __imag__ z += 2;
  double* p = &__imag__ z;
  (void)p;
  ++z;
  i--;
}
// Traits (clang Type::isArithmeticType/isFloatingType include complex
// types; isIntegralType and isSignedIntegerType do not).
static_assert(__is_arithmetic(C) && __is_fundamental(C) && __is_arithmetic(CI));
static_assert(__is_floating_point(C) && !__is_integral(CI) && !__is_compound(C));
static_assert(!__is_signed(CI) && !__is_unsigned(CI));
static_assert(__is_scalar(C) && __is_literal_type(C) && __is_trivial(C));
static_assert(__is_aggregate(C));
static_assert(__is_trivially_copyable(CI) && __is_standard_layout(CF));
// Conversions in initialization and calls.
double take(double);
C cf(CF f) { return f; }
CI ci(C d) { return d; }
bool tb(C z) { return z; }
double drop(C z) { return __real__ z + take(__imag__ z); }
C bc = __builtin_complex(1.0, 2.0);
CF lst = {1.0f, 2.0f};
C ex = C(3);
