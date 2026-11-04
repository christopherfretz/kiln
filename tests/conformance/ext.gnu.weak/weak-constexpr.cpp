// [expr.const]/10: an equality comparison whose result is unspecified is
// not a core constant expression. The address of a weak declaration may
// be null at run time, so comparing it, or converting it to bool, cannot
// be folded (clang `ExprConstant`'s `isWeakDecl`: "comparison against
// address of weak declaration"), while it stays a valid constant
// initializer of a pointer. libstdc++'s gthr-posix.h
// `__gthread_active_p` initializes a `static void *const` with the address
// of a weakref and tests it at run time.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:16 error "constexpr variable 'b1' must be initialized by a constant expression"
//@ diag: 25:25 note "comparison against address of weak declaration '&wf' can only be performed at runtime"
//@ diag: 26:16 error "constexpr variable 'b2' must be initialized by a constant expression"
//@ diag: 26:25 note "comparison against address of weak declaration '&wv' can only be performed at runtime"
//@ diag: 27:16 error "constexpr variable 'b3' must be initialized by a constant expression"
//@ diag: 27:25 note "comparison against address of weak declaration '&rf' can only be performed at runtime"
//@ diag: 28:16 error "constexpr variable 'b4' must be initialized by a constant expression"

extern "C" void wf() __attribute__((weak));
extern "C" int wv __attribute__((weak));
static void rf() __attribute__((weakref("wf")));
void strong();

constexpr void (*p)() = &wf;                 // OK: a relocation
constexpr bool b1 = &wf != nullptr;
constexpr bool b2 = &wv == nullptr;
constexpr bool b3 = &rf == nullptr;
constexpr bool b4 = wf;
constexpr bool b5 = &strong != nullptr;      // OK
static_assert(b5);
