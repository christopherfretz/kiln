// [expr.yield]/1: `co_yield e` is `co_await promise.yield_value(e)`;
// overload resolution picks `yield_value` for the operand, and a
// braced-init-list operand is allowed. The value of the expression is the
// awaiter's `await_resume` result.
//@ expect: run
//@ std: c++20
//@ stdout: int 1 -> 2
//@ stdout: double 2.5 -> 3
//@ stdout: pair 4 5 -> 10
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct pair {
  int a, b;
};

struct ret_aw {
  int v;
  bool await_ready() { return true; }
  void await_suspend(std::coroutine_handle<>) {}
  int await_resume() { return v; }
};

struct job {
  struct promise_type {
    job get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
    ret_aw yield_value(int v) {
      printf("int %d -> ", v);
      return {v + 1};
    }
    ret_aw yield_value(double v) {
      printf("double %.1f -> ", v);
      return {3};
    }
    ret_aw yield_value(pair p) {
      printf("pair %d %d -> ", p.a, p.b);
      return {10};
    }
  };
};

job run() {
  printf("%d\n", co_yield 1);
  printf("%d\n", co_yield 2.5);
  printf("%d\n", co_yield {4, 5});
}

int main() { run(); }
