// [expr.await]/2: an await-expression shall appear only in a
// potentially-evaluated expression.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:16 error "'co_await' cannot be used in an unevaluated context"
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

job f() {
  (void)sizeof(co_await std::suspend_never{});
  co_return;
}
