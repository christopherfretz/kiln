// [dcl.constexpr]/3 (C++20), [dcl.fct.def.coroutine]/1: a constexpr
// function shall not be a coroutine.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:24 error "'co_return' cannot be used in a constexpr function"
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
constexpr job make() { co_return; }
