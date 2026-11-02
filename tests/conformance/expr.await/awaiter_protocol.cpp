// [expr.await]/5: `await_ready` is called first; if false, `await_suspend`
// runs with the coroutine's handle. A `void` result suspends, `false`
// resumes immediately, a `coroutine_handle` result is resumed (symmetric
// transfer). `await_resume` gives the value of the co_await expression.
//@ expect: run
//@ std: c++20
//@ stdout: ready
//@ stdout: v 1
//@ stdout: bool false
//@ stdout: v 2
//@ stdout: void suspend
//@ stdout: main resumes
//@ stdout: v 3
//@ stdout: other runs
//@ stdout: v 4
//@ stdout: end
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct ready_aw {
  bool await_ready() { printf("ready\n"); return true; }
  void await_suspend(std::coroutine_handle<>) { printf("unreachable\n"); }
  int await_resume() { return 1; }
};

struct bool_aw {
  bool await_ready() { return false; }
  bool await_suspend(std::coroutine_handle<>) { printf("bool false\n"); return false; }
  int await_resume() { return 2; }
};

std::coroutine_handle<> resumer;

struct void_aw {
  bool await_ready() { return false; }
  void await_suspend(std::coroutine_handle<> h) { printf("void suspend\n"); resumer = h; }
  int await_resume() { return 3; }
};

struct job {
  struct promise_type {
    job get_return_object() { return job{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
};

std::coroutine_handle<> other_h;

struct handle_aw {
  bool await_ready() { return false; }
  std::coroutine_handle<> await_suspend(std::coroutine_handle<> self) {
    resumer = self;
    return other_h;
  }
  int await_resume() { return 4; }
};

// `other` must start suspended so the transfer is what runs it.
struct lazy {
  struct promise_type {
    lazy get_return_object() { return lazy{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
};

lazy other2() {
  printf("other runs\n");
  co_return;
}

job run() {
  printf("v %d\n", co_await ready_aw{});
  printf("v %d\n", co_await bool_aw{});
  printf("v %d\n", co_await void_aw{});
  int v = co_await handle_aw{};
  printf("v %d\n", v);
}

int main() {
  lazy o = other2();
  other_h = o.h;
  job j = run();
  printf("main resumes\n");
  resumer.resume();
  // run() transferred to other2, which suspended at its final point and
  // returned here; resume run() to finish.
  resumer.resume();
  printf("end\n");
  o.h.destroy();
  j.h.destroy();
}
