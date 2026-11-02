// [expr.prim.id.dtor]/2, [except.spec]/6, [expr.unary.noexcept]/2: a
// pseudo-destructor call is not potentially-throwing; like clang, kiln does
// not look at its object expression either, so `noexcept(f().~I())` is true
// even for a potentially-throwing `f`. A class type with a
// potentially-throwing destructor stays potentially-throwing, and a scalar
// destructor call satisfies a `{ t.~T() } noexcept` requirement.
// Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:15 error "static assertion failed"
//@ diag: 23:15 error "static assertion failed"
//@ diag: 23:15 note "because 'S' does not satisfy 'B'"
//@ diag: 16:59 note "may throw an exception"
int &f();
using I = int;
template<class T> concept B = requires(T &t) { { t.~T() } noexcept; };
struct S { ~S() noexcept(false); };
struct U { ~U(); };
static_assert(noexcept(f().~I()));
static_assert(B<int> && B<int *> && B<const char *> && B<U>);
static_assert(!B<S>);
static_assert(!noexcept(f().~I()));
static_assert(B<S>);
