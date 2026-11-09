// gcov coverage (WP-941) of a coroutine: as clang, each private
// `await_suspend` wrapper gets an artificial `DISubprogram` (linkage name
// only, in the main file) and its calls the suspend expression's location,
// so GCOVProfiling instruments it and the `co_await`/`co_yield` lines
// count their suspensions; the coroutine's `llvm.coro.id` is located at the
// body's `{`; the cleanup-exit switch and the shared `unreachable` block
// carry no location (clang appends them without the builder). The shim
// has no conversion operator and no non-trivial implicit constructor.
//@ args: --coverage -Xclang -disable-llvm-passes
namespace std {
template <class R, class... A> struct coroutine_traits { using promise_type = typename R::promise_type; };
template <class P> struct coroutine_handle {
  void *p;
  static coroutine_handle from_address(void *a) noexcept { coroutine_handle h; h.p = a; return h; }
  static coroutine_handle from_promise(P &pr) noexcept {
    return from_address(__builtin_coro_promise(&pr, alignof(P), true));
  }
  void *address() const noexcept { return p; }
  P &promise() const { return *static_cast<P *>(__builtin_coro_promise(p, alignof(P), false)); }
};
struct suspend_always {
  bool await_ready() const noexcept { return false; }
  template <class P> void await_suspend(coroutine_handle<P>) const noexcept {}
  void await_resume() const noexcept {}
};
} // namespace std

struct gen {
  struct promise_type {
    int cur;
    gen get_return_object() { return {std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    std::suspend_always yield_value(int v) noexcept {
      cur = v;
      return {};
    }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
};

gen count(int n) {
  co_await std::suspend_always{};
  for (int i = 0; i < n; ++i)
    if (i % 2)
      co_yield i * 10;
  co_yield -1;
}
