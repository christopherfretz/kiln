// [expr.await]/5, [class.temporary]/4: temporaries of the full-expression
// that contains a `co_await` live across the suspension and are destroyed
// at the end of the full-expression, after resumption.
//@ expect: run
//@ std: c++20
//@ stdout: make 1
//@ stdout: suspended
//@ stdout: got 1
//@ stdout: dtor 1
//@ stdout: after
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct tmp {
  int v;
  tmp(int v) : v(v) { printf("make %d\n", v); }
  ~tmp() { printf("dtor %d\n", v); }
};

std::coroutine_handle<> resumer;

struct aw {
  int v;
  bool await_ready() { return false; }
  void await_suspend(std::coroutine_handle<> h) { resumer = h; }
  int await_resume() { return v; }
};

aw wait_on(const tmp &t) { return aw{t.v}; }

struct job {
  struct promise_type {
    job get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

int take(int v) {
  printf("got %d\n", v);
  return v;
}

job run() {
  take(co_await wait_on(tmp(1)));
  printf("after\n");
}

int main() {
  run();
  printf("suspended\n");
  resumer.resume();
}
