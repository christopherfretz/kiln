// [dcl.constexpr]/3, [expr.const]/10: a coroutine is never constexpr (a
// coroutine lambda is not implicitly constexpr), so calling one, or a
// coroutine builtin, is not a constant expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:15 error "static assertion expression is not an integral constant expression"
//@ diag: 22:16 note "non-constexpr function 'operator()' cannot be used in a constant expression"
//@ diag: 21:20 note "declared here"
//@ diag: 23:21 error "constexpr variable 'b' must be initialized by a constant expression"
#include "Inputs/coro.h"

struct job {
  struct promise_type {
    constexpr job get_return_object() { return {}; }
    constexpr std::suspend_never initial_suspend() noexcept { return {}; }
    constexpr std::suspend_never final_suspend() noexcept { return {}; }
    constexpr void return_void() {}
    void unhandled_exception() {}
  };
};
constexpr auto l = []() -> job { co_return; };
static_assert((l(), true));
constexpr bool b = (__builtin_coro_noop(), true);
