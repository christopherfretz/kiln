// [bit.cast]/2 (the working draft, P4101R1): "Constant When: Neither To nor
// From has constexpr-unknown representation", and `std::meta::info` has
// constexpr-unknown representation ([expr.const.core]/1.2). A bit cast
// from or to `info`, or a class holding one, is well-formed (`info` is
// scalar, so trivially copyable; bit-cast-info-run.cpp) but never a
// constant expression: in every manifestly constant-evaluated context, in
// the non-dependent spelling and once instantiated. kiln's wording (clang
// has no such note); GCC 16.1 makes each cast ill-formed instead ("cannot
// be used with consteval-only types"; docs/status/deviations.md).
//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19/22 do not implement P2996 reflection
//@ strict-diags
//@ diag: 52:16 error "constexpr variable 'c1' must be initialized by a constant expression"
//@ diag: 52:21 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 53:13 error "constexpr variable 'c2' must be initialized by a constant expression"
//@ diag: 53:18 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 50:12 note "invalid type 'info' is a member of 'S'"
//@ diag: 54:16 error "constexpr variable 'c3' must be initialized by a constant expression"
//@ diag: 54:21 note "bit_cast to a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 55:15 error "static assertion expression is not an integral constant expression"
//@ diag: 55:15 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 57:3 error "non-type template argument is not a constant expression"
//@ diag: 57:3 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 58:37 error "case value is not a constant expression"
//@ diag: 58:37 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 59:20 error "noexcept specifier argument is not a constant expression"
//@ diag: 59:20 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 60:30 error "substitution into constraint expression resulted in a non-constant expression"
//@ diag: 61:14 note "while checking constraint satisfaction for template 'rq<info>' required here"
//@ diag: 61:14 error "no matching function for call to 'rq'"
//@ diag: 60:71 note "candidate template ignored: constraints not satisfied [with T = info]"
//@ diag: 62:57 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 63:11 note "in instantiation of function template specialization 'ft<std::meta::info>' requested here"
//@ diag: 62:61 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 64:53 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 65:20 note "in instantiation of static data member 'M<std::meta::info>::v' requested here"
//@ diag: 64:57 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 71:11 error "call to consteval function 'cf' is not a constant expression"
//@ diag: 70:30 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 71:11 note "in call to 'cf()'"
//@ diag: 66:59 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 66:42 note "while substituting into a lambda expression here"
//@ diag: 67:11 note "in instantiation of function template specialization 'lt<std::meta::info>' requested here"
//@ diag: 66:63 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
//@ diag: 68:83 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 69:19 note "in instantiation of function template specialization 'O<int>::f<std::meta::info>' requested here"
//@ diag: 68:87 note "bit_cast from a type with constexpr-unknown representation ('std::meta::info') is not allowed in a constant expression"
using info = decltype(^^int);
struct S { info r; };
struct L { long r; };
constexpr long c1 = __builtin_bit_cast(long, info{});
constexpr L c2 = __builtin_bit_cast(L, S{});
constexpr info c3 = __builtin_bit_cast(info, 0L);
static_assert(__builtin_bit_cast(long, info{}) == 0);
template <long N> struct I {};
I<__builtin_bit_cast(long, info{})> i1;
void sw(long x) { switch (x) { case __builtin_bit_cast(long, info{}): break; } }
void ne() noexcept(__builtin_bit_cast(long, info{}) == 0);
template <class T> requires (__builtin_bit_cast(long, T{}) == 0) void rq(T) {}
void urq() { rq(info{}); }
template <class T> constexpr long ft() { constexpr long v = __builtin_bit_cast(long, T{}); return v; }
long u1 = ft<info>();
template <class T> struct M { static constexpr long v = __builtin_bit_cast(long, T{}); };
long u2 = M<info>::v;
template <class T> long lt() { return [] { constexpr long v = __builtin_bit_cast(long, T{}); return v; }(); }
long u3 = lt<info>();
template <class T> struct O { template <class U> static long f() { constexpr long v = __builtin_bit_cast(long, U{}); return v; } };
long u4 = O<int>::f<info>();
consteval long cf() { return __builtin_bit_cast(long, info{}); }
long u5 = cf();
