// [except.spec]/5 with clang's C-library allowance (CheckEquivalentExceptionSpec):
// a redeclaration that omits the non-throwing exception specification of a
// previous declaration with C language linkage in a system header (or of a
// library builtin) is accepted silently and inherits `throw()`, as libc++'s
// `extern "C" double lgamma_r(double, int*);` redeclares glibc's (WP-268).
//@ expect: compile-ok
//@ strict-diags
//@ args: -isystem %{test_dir}/Inputs
#include <redecl_sys.h>

extern "C" double sys_lgamma_r(double, int*);
extern "C" double sys_throw(double);
extern "C" double sys_noexcept_true(double);
extern "C" double sys_potentially_throwing(double);
static_assert(noexcept(sys_lgamma_r(1.0, nullptr)));
static_assert(noexcept(sys_throw(1.0)));
static_assert(noexcept(sys_noexcept_true(1.0)));
static_assert(!noexcept(sys_potentially_throwing(1.0)));

// A recognized library builtin counts as declared by the implementation.
extern "C" std::size_t strlen(const char*) noexcept;
extern "C" std::size_t strlen(const char*);
static_assert(noexcept(strlen("")));

// The previous declaration in the system header was never diagnosed.
static_assert(noexcept(operator new(1, std::nothrow_t{})));

double call(int* s) { return sys_lgamma_r(2.0, s); }
