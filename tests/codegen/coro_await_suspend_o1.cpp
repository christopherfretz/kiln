//@ args: -O1
// [expr.await]/5.1, [dcl.fct.def.coroutine]: at -O1 the always-inline
// await_suspend wrappers (void, bool and handle forms) are inlined into
// the split coroutine; the resulting awaiter, handle and frame accesses
// carry clang's TBAA. coro_await_suspend_o1_raw.cpp checks the wrappers
// themselves before LLVM passes.
namespace std {
template <class R, class... A> struct coroutine_traits { using promise_type = typename R::promise_type; };
template <class P = void> struct coroutine_handle;
template <> struct coroutine_handle<void> {
  void *p = nullptr;
  static coroutine_handle from_address(void *a) noexcept { coroutine_handle h; h.p = a; return h; }
  void *address() const noexcept { return p; }
  void resume() const { __builtin_coro_resume(p); }
  void destroy() const { __builtin_coro_destroy(p); }
};
template <class P> struct coroutine_handle {
  void *p = nullptr;
  static coroutine_handle from_address(void *a) noexcept { coroutine_handle h; h.p = a; return h; }
  static coroutine_handle from_promise(P &pr) noexcept {
    return from_address(__builtin_coro_promise(&pr, alignof(P), true));
  }
  operator coroutine_handle<>() const noexcept { return coroutine_handle<>::from_address(p); }
  void *address() const noexcept { return p; }
  P &promise() const { return *static_cast<P *>(__builtin_coro_promise(p, alignof(P), false)); }
};
struct suspend_always {
  bool await_ready() const noexcept { return false; }
  void await_suspend(coroutine_handle<>) const noexcept {}
  void await_resume() const noexcept {}
};
} // namespace std

struct task {
  struct promise_type {
    std::coroutine_handle<> next;
    struct fin {
      bool await_ready() noexcept { return false; }
      std::coroutine_handle<> await_suspend(std::coroutine_handle<promise_type> h) noexcept {
        return h.promise().next;
      }
      void await_resume() noexcept {}
    };
    task get_return_object() { return {std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    fin final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
  bool await_ready() { return false; }
  std::coroutine_handle<> await_suspend(std::coroutine_handle<> c) {
    h.promise().next = c;
    return h;
  }
  void await_resume() {}
};

struct maybe {
  bool await_ready() { return false; }
  bool await_suspend(std::coroutine_handle<>) noexcept { return true; }
  int await_resume() { return 1; }
};
struct throwing {
  bool await_ready() { return false; }
  void await_suspend(std::coroutine_handle<>) {}
  void await_resume() {}
};

task leaf() { co_return; }

task chain(int n) {
  int k = co_await maybe{};
  co_await throwing{};
  if (n + k)
    co_await leaf();
}
