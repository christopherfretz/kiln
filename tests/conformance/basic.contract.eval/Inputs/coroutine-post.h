// The coroutines of the run-coroutine-post-*.cpp tests ([dcl.contract.func],
// [dcl.fct.def.coroutine]/7, P2900R14 §3.5.2): a postcondition of a
// coroutine is evaluated once, when the coroutine first returns to its
// caller (the ramp returns), and its result name denotes the return
// object initialised from get_return_object(). `lazy` and `conv` suspend
// initially (the body has not run); `ea` runs to its first suspension
// first; `bad`'s postcondition is false (it ran to its final suspend).
#include <contracts>
#include <coroutine>
#include <cstdio>
#include <cstdlib>
#include <exception>
void handle_contract_violation(const std::contracts::contract_violation& v) {
  std::printf("handler %d\n", int(v.kind()));
}
bool note(const char* s, bool b) {
  std::printf("%s\n", s);
  return b;
}
template <bool Handle> struct task {
  struct promise_type;
  using handle = std::coroutine_handle<promise_type>;
  struct promise_type {
    int value = 0;
    auto get_return_object() {
      std::puts("gro");
      if constexpr (Handle)
        return handle::from_promise(*this);
      else
        return task{handle::from_promise(*this)};
    }
    std::suspend_always initial_suspend() noexcept {
      std::puts("initial");
      return {};
    }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_value(int v) { value = v; }
    void unhandled_exception() {}
  };
  handle h;
  task(handle h) : h(h) {}
  task(task&& o) : h(o.h) { o.h = {}; }
  ~task() {
    if (h) h.destroy();
  }
  int run() {
    while (!h.done()) h.resume();
    return h.promise().value;
  }
};
struct eager {
  struct promise_type {
    eager get_return_object() { return {std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
};
// get_return_object() returns the return type.
task<false> lazy(const int& x) pre(note("pre", x > 0)) post(r: note("post", r.h && !r.h.done())) {
  std::puts("body");
  co_return x * 2;
}
// get_return_object() returns another type, converted at the return.
task<true> conv(const int& x)
  post(r: note("post conv", !r.h.done())) {
  co_return x + 1;
}
eager ea() post(r: note("post eager", !r.h.done())) {
  std::puts("eager 1");
  co_await std::suspend_always{};
  std::puts("eager 2");
}
eager bad() post(r: note("post bad", !r.h.done())) {
  co_return;
}
int main() {
  {
    auto t = lazy(3);
    std::puts("returned");
    std::printf("%d\n", t.run());
    auto c = conv(4);
    std::printf("%d\n", c.run());
  }
  {
    auto e = ea();
    std::puts("returned eager");
    e.h.resume();
    e.h.destroy();
  }
  std::set_terminate([] {
    std::printf("terminate\n");
    std::fflush(stdout);
    std::_Exit(7);
  });
  auto b = bad();
  std::puts("after bad");
  b.h.destroy();
}
