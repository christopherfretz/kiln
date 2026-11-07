// [expr.await]/5.7, [conv.lval]: a co_await whose `await_resume` returns
// a reference is a glvalue; initializing a non-reference variable or
// passing it by value converts it to the referee's value, not to its
// address, on the resumed path and on the ready path, with an awaiter
// temporary whose cleanups the full-expression shares.
//@ expect: run
//@ std: c++20
//@ stdout: 42 43 44 45 46
#include <coroutine>
#include <cstdio>
#include <utility>

struct task {
  struct promise_type {
    task get_return_object() {
      return {std::coroutine_handle<promise_type>::from_promise(*this)};
    }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_value(long v) { result = v; }
    void unhandled_exception() {}
    long result = 0;
  };
  std::coroutine_handle<promise_type> h;
};

std::coroutine_handle<> pending;
int storage = 0;

struct awaiter {
  int &v;
  bool ready;
  bool await_ready() noexcept { return ready; }
  void await_suspend(std::coroutine_handle<> h) noexcept { pending = h; }
  int &&await_resume() { return std::move(v); }
};
struct lv_awaiter {
  int &v;
  bool await_ready() noexcept { return false; }
  void await_suspend(std::coroutine_handle<> h) noexcept { pending = h; }
  int &await_resume() { return v; }
  ~lv_awaiter() {}
};
struct source {
  awaiter operator co_await() { return awaiter{storage, false}; }
};
source src;
long by_value(long x) { return x; }

task f() {
  auto ret = co_await src;
  co_return ret;
}
task g() {
  int ret = co_await lv_awaiter{storage};
  co_return ret;
}
task h() { co_return by_value(co_await awaiter{storage, false}); }
task k() { co_return co_await awaiter{storage, true} + 1; }

int main() {
  task a = f();
  storage = 42;
  pending.resume();
  task b = g();
  storage = 43;
  pending.resume();
  task c = h();
  storage = 44;
  pending.resume();
  storage = 44;
  task d = k();
  task e = g();
  storage = 46;
  pending.resume();
  printf("%ld %ld %ld %ld %ld\n", a.h.promise().result, b.h.promise().result,
         c.h.promise().result, d.h.promise().result, e.h.promise().result);
  a.h.destroy();
  b.h.destroy();
  c.h.destroy();
  d.h.destroy();
  e.h.destroy();
}
