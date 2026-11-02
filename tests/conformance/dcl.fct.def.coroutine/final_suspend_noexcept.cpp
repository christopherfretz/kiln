// [dcl.fct.def.coroutine]/15: the expression co_await
// promise.final_suspend() shall not be potentially-throwing.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:5 error "the expression 'co_await __promise.final_suspend()' is required to be non-throwing"
//@ diag: 19:37 note "must be declared with 'noexcept'"
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct aw {
  bool await_ready() noexcept;
  void await_suspend(std::coroutine_handle<>) noexcept;
  void await_resume() noexcept;
};

struct job {
  struct promise_type {
    job get_return_object();
    std::suspend_never initial_suspend() noexcept;
    aw final_suspend() { return {}; }
    void return_void();
    void unhandled_exception();
  };
};

job f() { co_return; }
