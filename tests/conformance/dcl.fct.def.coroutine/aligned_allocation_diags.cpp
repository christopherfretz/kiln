// [dcl.fct.def.coroutine]/9-10 with -fcoro-aligned-allocation (clang's
// P2014R0 option 2): a promise `operator new` without an `align_val_t`
// parameter still wins over the global aligned form, with a warning at
// its declaration; under `get_return_object_on_allocation_failure` the
// global form needed is `operator new(size_t, align_val_t, nothrow_t)`.
//@ expect: compile-fail
//@ std: c++20
//@ args: -fcoro-aligned-allocation
//@ strict-diags
//@ diag: 22:16 warning "under -fcoro-aligned-allocation, the non-aligned allocation function for the promise type 'plain' has higher precedence than the global aligned allocation function"
//@ diag: 32:14 error "unable to find '::operator new(size_t, align_val_t, nothrow_t)' for 'nothrow'"
#include "Inputs/coro.h"

template <class T> struct promise_base {
  T get_return_object() { return {}; }
  std::suspend_never initial_suspend() noexcept { return {}; }
  std::suspend_never final_suspend() noexcept { return {}; }
  void return_void() {}
  void unhandled_exception() {}
};
struct plain_task { struct promise_type : promise_base<plain_task> {
  static void *operator new(decltype(sizeof 0) n) { return ::operator new(n); }
}; };
plain_task plain() { co_return; }

struct nothrow_task {
  struct promise_type : promise_base<nothrow_task> {
    static nothrow_task get_return_object_on_allocation_failure() { return {}; }
  };
};
// coro.h declares no aligned nothrow `operator new`.
nothrow_task nothrow() { co_return; }
