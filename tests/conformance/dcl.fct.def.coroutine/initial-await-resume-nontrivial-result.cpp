// [dcl.fct.def.coroutine]/5, [class.temporary]/4: the initial await's
// `await_resume()` runs inside the implicit try-block around the body; a
// class-type result with a non-trivial destructor is a temporary destroyed
// at the end of that full-expression, still inside the handler's scope.
// kiln's codegen left the destructor cleanup open past the end of the try
// and crashed (WP-971). The twin `plain` resumes with a trivial result.
//@ expect: run
//@ std: c++20
//@ strict-diags
//@ stdout: resume 1
//@ stdout: ~R 1
//@ stdout: body 1
//@ stdout: resume 0
//@ stdout: body 0
//@ stdout: done
#include "Inputs/coro.h"

struct R {
  int v;
  ~R() { printf("~R %d\n", v); }
};

struct init_awaiter {
  bool await_ready() noexcept { return true; }
  void await_suspend(std::coroutine_handle<>) noexcept {}
  R await_resume() {
    printf("resume 1\n");
    return R{1};
  }
};

struct plain_awaiter {
  bool await_ready() noexcept { return true; }
  void await_suspend(std::coroutine_handle<>) noexcept {}
  int await_resume() {
    printf("resume 0\n");
    return 0;
  }
};

template <class Init> struct co_task {
  struct promise_type {
    co_task get_return_object() { return {}; }
    Init initial_suspend() { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

co_task<init_awaiter> nontrivial() {
  printf("body 1\n");
  co_return;
}

co_task<plain_awaiter> plain() {
  printf("body 0\n");
  co_return;
}

int main() {
  nontrivial();
  plain();
  printf("done\n");
}
