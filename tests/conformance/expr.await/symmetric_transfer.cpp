// [expr.await]/5.1.1: when `await_suspend` returns a coroutine_handle `h`,
// the awaiting coroutine is suspended and `h.resume()` is evaluated; the
// resumption must not grow the stack (clang/LLVM lower it as a `musttail`
// call of the resume function through `llvm.coro.await.suspend.handle`).
// A million hops between two coroutines would overflow any stack without
// the tail call.
//@ expect: run
//@ std: c++20
//@ stdout: 1000000
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct xtask {
  struct promise_type {
    std::coroutine_handle<> cont;
    xtask get_return_object() noexcept {
      return xtask{std::coroutine_handle<promise_type>::from_promise(*this)};
    }
    std::suspend_always initial_suspend() noexcept { return {}; }
    struct fin {
      bool await_ready() noexcept { return false; }
      std::coroutine_handle<> await_suspend(std::coroutine_handle<promise_type> h) noexcept {
        auto c = h.promise().cont;
        return c ? c : std::noop_coroutine();
      }
      void await_resume() noexcept {}
    };
    fin final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
  explicit xtask(std::coroutine_handle<promise_type> c) : h(c) {}
  xtask(xtask&& o) : h(o.h) { o.h = {}; }
  ~xtask() { if (h) h.destroy(); }
  bool await_ready() { return false; }
  std::coroutine_handle<> await_suspend(std::coroutine_handle<> c) {
    h.promise().cont = c;
    return h;
  }
  void await_resume() {}
};

int hops = 0;
xtask leaf() { ++hops; co_return; }
xtask chain(int n) {
  for (int i = 0; i < n; ++i) co_await leaf();
}

int main() {
  xtask t = chain(1000000);
  t.h.resume();
  printf("%d\n", hops);
  return t.h.done() && hops == 1000000 ? 0 : 1;
}
