// [dcl.fct.def.coroutine]/6: a promise type shall not declare both
// return_void and return_value.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:5 error "the coroutine promise type 'promise_type' declares both 'return_value' and 'return_void'"
//@ diag: 15:10 note "member 'return_void' first declared here"
//@ diag: 16:10 note "member 'return_value' first declared here"
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct job {
  struct promise_type {
    job get_return_object();
    std::suspend_never initial_suspend() noexcept;
    std::suspend_never final_suspend() noexcept;
    void return_void();
    void return_value(int);
    void unhandled_exception();
  };
};
job f() { co_return; }
