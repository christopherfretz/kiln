// [meta.reflection.queries] constant_of's example: a reflection of a
// constexpr local escaping its call reads the variable outside its
// lifetime. The answer must not depend on whether the call was evaluated
// before (a memoized call would skip the frame that makes the escape
// visible): the same call evaluated first on its own, then under
// `constant_of`/`extract`, also through a caller that only passes the
// reflection on. A call that takes no reflection of a local stays
// memoized. GCC 16.1 accepts every line (it caches the call too).
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ strict-diags
//@ oven: 43 memo_hits<=0
//@ oven: 44 memo_hits>=1
//@ diag: 38:16 error "constexpr variable 'r1' must be initialized by a constant expression"
//@ diag: 38:21 note "read of variable 'x' outside its lifetime is not allowed in a constant expression"
//@ diag: 38:21 error "call to consteval function 'std::meta::constant_of' is not a constant expression"
//@ diag: 38:21 note "read of variable 'x' outside its lifetime is not allowed in a constant expression"
//@ diag: 40:15 error "constexpr variable 'v1' must be initialized by a constant expression"
//@ diag: 40:20 note "'^^x' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 32:40 note "declared here"
//@ diag: 40:20 error "call to consteval function 'std::meta::extract<int>' is not a constant expression"
//@ diag: 40:20 note "'^^x' is not a variable of static storage duration, nor one whose lifetime began within the evaluation"
//@ diag: 32:40 note "declared here"
//@ diag: 42:16 error "constexpr variable 'r2' must be initialized by a constant expression"
//@ diag: 42:21 note "read of variable 'x' outside its lifetime is not allowed in a constant expression"
//@ diag: 42:21 error "call to consteval function 'std::meta::constant_of' is not a constant expression"
//@ diag: 42:21 note "read of variable 'x' outside its lifetime is not allowed in a constant expression"
#include <meta>
using std::meta::info;

consteval info fn(int) { constexpr int x = 42; return ^^x; }
consteval info pass(int n) { return fn(n); }
consteval int square(int n) { return n * n; }

constexpr info w1 = fn(1);
constexpr info w2 = pass(2);
constexpr info r1 = constant_of(fn(1));
constexpr info w3 = fn(3);
constexpr int v1 = std::meta::extract<int>(fn(3));
static_assert(fn(1) == w1);
constexpr info r2 = constant_of(pass(2));
static_assert(square(5) == 25);
static_assert(square(5) == 25);

