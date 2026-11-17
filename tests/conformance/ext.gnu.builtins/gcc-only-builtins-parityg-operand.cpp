// GCC's __builtin_parityg takes an unsigned integer type that is neither
// `bool` nor an enumeration (GCC 16.1 "has boolean type", "has enumerated
// type", "has signed type"); clang has no such builtin, so kiln follows
// GCC, with an error in clang's style. A dependent operand is checked at
// instantiation. Found running GCC's C++ testsuite. WP-1612.
//@ std: c++20
//@ expect: compile-fail
//@ requires: clang>=22
//@ oracle: gcc
//@ skip(clang): clang 19/22 have no __builtin_parityg
//@ strict-diags
//@ diag: 21:60 error "1st argument must be a scalar unsigned integer type (was 'bool')"
//@ diag: 29:5 note "in instantiation of function template specialization 'par<bool>' requested here"
//@ diag: 24:23 error "1st argument must be a scalar unsigned integer type (was 'bool')"
//@ diag: 25:23 error "1st argument must be a scalar unsigned integer type (was 'bool')"
//@ diag: 26:23 error "1st argument must be a scalar unsigned integer type (was 'E')"
//@ diag: 27:23 error "1st argument must be a scalar unsigned integer type (was 'C')"
//@ diag: 28:23 error "1st argument must be a scalar unsigned integer type (was 'int')"
enum E : unsigned { e1 = 1 };
enum class C : unsigned { c1 };
template <class T> int par(T x) { return __builtin_parityg(x); }
template <class T> int unused(T x) { return __builtin_parityg(x); }
void t(bool b) {
    __builtin_parityg(b);
    __builtin_parityg(true);
    __builtin_parityg(e1);
    __builtin_parityg(C::c1);
    __builtin_parityg(-1);
    par(b);
}
static_assert(__builtin_parityg(7u) == 1);
static_assert(__builtin_parityg((unsigned char)3) == 0);
static_assert(__builtin_parityg((unsigned __int128)1 << 100) == 1);
int ok = par(5u) + par((unsigned short)1);
