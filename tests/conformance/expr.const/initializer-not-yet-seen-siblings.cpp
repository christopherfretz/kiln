// [expr.const]/2: the value of a const enumeration or integral variable is
// fixed where its initializer is; one reading a variable whose initializer
// follows stays not constant (WP-948 siblings of initializer-not-yet-seen).
//@ std: c++23
//@ expect: compile-fail
//@ diag: 16:13 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 16:17 note "initializer of 'b' is not a constant expression"
//@ diag: 14:9 note "declared here"
//@ diag: 20:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:15 note "initializer of 'l2' is not a constant expression"
//@ diag: 18:12 note "declared here"
enum class E { x = 5 };
extern const E a;
const E b = a;
const E a = E::x;
constexpr E c = b;
namespace n { extern const long l; }
const long l2 = n::l * 2;
const long n::l = 4;
static_assert(l2 == 8);
