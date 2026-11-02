// [class.static.data]/4, [expr.const]/5: a `static const` data member of
// integral or enumeration type with an in-class initializer needs a constant
// initializer, whatever the width of its value (an `unsigned __int128` above
// INT128_MAX, a negative `__int128`, enums, bool), including in a class
// template instantiation; a non-constant initializer is an error.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 24:38 error "in-class initializer for static data member is not a constant expression"
//@ diag: 25:30 error "in-class initializer for static data member is not a constant expression"
//@ requires: clang<22
// clang 22 emits error: in-class initializer for static data member is not a constant expression; WP-355 residual
enum En { ea = 5 };
enum class Ec : long long { x = 7 };
template<typename V> struct traits {
  static const bool is_signed = (V)(-1) < 0;
  static const int digits = sizeof(V) * 8 - is_signed;
  static const V max = is_signed ? (((((V)1 << (digits - 1)) - 1) << 1) + 1) : ~(V)0;
  static const V min = is_signed ? -max - 1 : (V)0;
};
unsigned __int128 rt();
__int128 g;
struct U {
  static const unsigned __int128 a = rt();
  static const __int128 a2 = g;
  static const unsigned __int128 b = ~(unsigned __int128)0 + 0;
  static const unsigned __int128 c = b >> 64;
  static const __int128 k = -(__int128)1 << 120;
  static const bool t = b != 0;
  static const En e = ea;
  static const Ec x = Ec::x;
};
static_assert(traits<unsigned __int128>::max == ~(unsigned __int128)0);
static_assert(traits<unsigned __int128>::digits == 128);
static_assert(traits<__int128>::min < 0 && traits<__int128>::max > 0);
static_assert(U::c == 0xffffffffffffffffull && U::k < 0 && U::t);
static_assert(U::e == ea && U::x == Ec::x);
