// [expr.await]/5.1, [dcl.fct.def.coroutine]: as clang 19, each
// `await_suspend` call is outlined into a private
// `<coroutine>.__await_suspend_wrapper__<init|final|awaitN|yieldN>(awaiter,
// frame)` called through `llvm.coro.await.suspend.void`/`.bool`/`.handle`
// (a nounwind call unless the suspend expression can throw; always an
// invoke for a handle). After CoroSplit (run at -O0 as clang does), the
// handle form resumes the returned coroutine with a `musttail` call
// (symmetric transfer).
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
  for (int i = 0; i < n + k; ++i)
    co_await leaf();
}
