// [dcl.fct.def.coroutine]/7 (P2014/CWG2563 as clang implements it): the
// result of `get_return_object()` initializes the returned object; when
// its type differs from the return type it is held in a variable and
// converted at the first return to the caller, after the initial suspend.
//@ expect: run
//@ std: c++20
//@ stdout: gro
//@ stdout: initial
//@ stdout: convert 5
//@ stdout: result 5
#include "Inputs/coro.h"

struct init_awaiter {
  bool await_ready() const noexcept { return false; }
  void await_suspend(std::coroutine_handle<>) const noexcept { printf("initial\n"); }
  void await_resume() const noexcept {}
};

struct result {
  int v;
};

struct handle_box {
  int v;
  operator result() const {
    printf("convert %d\n", v);
    return result{v};
  }
};

struct result_traits_promise {
  handle_box get_return_object() {
    printf("gro\n");
    return handle_box{5};
  }
  init_awaiter initial_suspend() noexcept { return {}; }
  std::suspend_always final_suspend() noexcept { return {}; }
  void return_void() {}
  void unhandled_exception() {}
};

template <> struct std::coroutine_traits<result> {
  using promise_type = result_traits_promise;
};

result make() { co_return; }

int main() {
  result r = make();
  printf("result %d\n", r.v);
}
