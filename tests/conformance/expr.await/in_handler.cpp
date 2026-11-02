// [expr.await]/2: an await-expression shall not appear in a handler.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 21:5 error "'co_await' cannot be used in the handler of a try block"
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
  try {
    co_await std::suspend_never{};
  } catch (...) {
    co_await std::suspend_never{};
  }
}
