// [dcl.constexpr]/3 (C++20), [dcl.fct.def.coroutine]/1: the call operator
// of a lambda declared consteval shall not be a coroutine, and a
// `co_return` is no return statement ("no return statement in consteval
// function", at the `[`). An implicitly constexpr lambda may be a coroutine
// (it is then not constexpr). kiln used to accept the first (WP-455).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 23:34 error "'co_return' cannot be used in a consteval function"
//@ diag: 23:10 error "no return statement in consteval function"

#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct job {
  struct promise_type {
    job get_return_object();
    std::suspend_never initial_suspend() noexcept;
    std::suspend_never final_suspend() noexcept;
    void return_void();
    void unhandled_exception();
  };
};
auto l = []() consteval -> job { co_return; };
auto m = []() -> job { co_return; };
