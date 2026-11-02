// [dcl.fct.def.coroutine]/5: an exception escaping the body (or the
// initial `await_resume`) is caught and `promise.unhandled_exception()` is
// called inside the handler; the coroutine then reaches its final suspend
// point. Locals of the body are destroyed by the unwinding.
//@ expect: run
//@ std: c++20
//@ stdout: dtor local
//@ stdout: caught 42
//@ stdout: final
//@ stdout: done 1
//@ stdout: resume threw 7
#include "Inputs/coro.h"

struct Local {
  ~Local() { printf("dtor local\n"); }
};

struct final_awaiter {
  bool await_ready() const noexcept { return false; }
  void await_suspend(std::coroutine_handle<>) const noexcept { printf("final\n"); }
  void await_resume() const noexcept {}
};

struct job {
  struct promise_type {
    int caught = 0;
    job get_return_object() { return job{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    final_awaiter final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {
      try {
        throw;
      } catch (int v) {
        caught = v;
        printf("caught %d\n", v);
      }
    }
  };
  std::coroutine_handle<promise_type> h;
};

job boom() {
  Local l;
  throw 42;
  co_return;
}

// An initial suspend whose `await_resume` throws: handled as if thrown by
// the body (the body is skipped).
struct throwing_init {
  bool await_ready() const noexcept { return true; }
  void await_suspend(std::coroutine_handle<>) const noexcept {}
  void await_resume() const { throw 7; }
};

struct job2 {
  struct promise_type {
    job2 get_return_object() { return job2{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    throwing_init initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {
      try {
        throw;
      } catch (int v) {
        printf("resume threw %d\n", v);
      }
    }
  };
  std::coroutine_handle<promise_type> h;
};

job2 skipped() {
  printf("body ran\n");
  co_return;
}

int main() {
  job j = boom();
  printf("done %d\n", j.h.done());
  j.h.destroy();
  job2 k = skipped();
  k.h.destroy();
}
