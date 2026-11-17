// The operand rules of GCC's __builtin_ffsg / __builtin_clrsbg (a signed
// integer, not bool or an enumeration), __builtin_parityg (unsigned),
// __builtin_speculation_safe_value (an integer or pointer first operand;
// a second of its type, or any pointer for a pointer; one or two
// arguments; never a constant expression) and __builtin_assoc_barrier
// (one scalar or vector). Errors in clang's style; GCC 16.1 rejects the
// same calls with its own wording. WP-1552.
//@ std: c++20
//@ expect: compile-fail
//@ requires: clang>=22
//@ oracle: gcc
//@ skip(clang): clang 19/22 have none of these builtins
//@ strict-diags
//@ diag: 36:20 error "1st argument must be a scalar signed integer type (was 'unsigned int')"
//@ diag: 37:20 error "1st argument must be a scalar signed integer type (was 'bool')"
//@ diag: 38:20 error "1st argument must be a scalar signed integer type (was 'E')"
//@ diag: 39:22 error "1st argument must be a scalar signed integer type (was 'double')"
//@ diag: 40:23 error "1st argument must be a scalar unsigned integer type (was 'int')"
//@ diag: 41:38 error "1st argument must be a scalar integer or pointer type (was 'double')"
//@ diag: 42:38 error "1st argument must be a scalar integer or pointer type (was 'bool')"
//@ diag: 43:38 error "1st argument must be a scalar integer or pointer type (was 'E')"
//@ diag: 44:38 error "1st argument must be a scalar integer or pointer type (was 'void ()')"
//@ diag: 45:41 error "second argument to '__builtin_speculation_safe_value' must have the type of the first, or both must be pointers ('long' vs 'int')"
//@ diag: 47:42 error "second argument to '__builtin_speculation_safe_value' must have the type of the first, or both must be pointers ('int' vs 'int *')"
//@ diag: 48:38 error "too few arguments to function call, expected at least 1, have 0"
//@ diag: 49:45 error "too many arguments to function call, expected at most 2, have 3"
//@ diag: 50:29 error "too few arguments to function call, expected 1, have 0"
//@ diag: 53:19 error "constexpr variable 'k' must be initialized by a constant expression"
enum E { e0 };
struct S {};
void fn();
int *ip;
const char *cp;

void t() {
    __builtin_ffsg(1u);
    __builtin_ffsg(true);
    __builtin_ffsg(e0);
    __builtin_clrsbg(1.0);
    __builtin_parityg(1);
    __builtin_speculation_safe_value(1.0);
    __builtin_speculation_safe_value(true);
    __builtin_speculation_safe_value(e0);
    __builtin_speculation_safe_value(fn);
    __builtin_speculation_safe_value(1, 2L);
    __builtin_speculation_safe_value(ip, cp);
    __builtin_speculation_safe_value(ip, 0);
    __builtin_speculation_safe_value();
    __builtin_speculation_safe_value(1, 2, 3);
    __builtin_assoc_barrier();
}

constexpr int k = __builtin_speculation_safe_value(1);
