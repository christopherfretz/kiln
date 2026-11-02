// A minimal <coroutine> ([support.coroutine]) for the coroutine corpus
// tests: self-contained, so kiln and the clang oracle see the same library.
#pragma once
namespace std {
template <class R, class... A> struct coroutine_traits {
  using promise_type = typename R::promise_type;
};
template <class P = void> struct coroutine_handle;
template <> struct coroutine_handle<void> {
  void* p = nullptr;
  static coroutine_handle from_address(void* a) noexcept {
    coroutine_handle h;
    h.p = a;
    return h;
  }
  void* address() const noexcept { return p; }
  explicit operator bool() const noexcept { return p != nullptr; }
  void operator()() const { resume(); }
  void resume() const { __builtin_coro_resume(p); }
  void destroy() const { __builtin_coro_destroy(p); }
  bool done() const { return __builtin_coro_done(p); }
};
template <class P> struct coroutine_handle {
  void* p = nullptr;
  static coroutine_handle from_address(void* a) noexcept {
    coroutine_handle h;
    h.p = a;
    return h;
  }
  static coroutine_handle from_promise(P& pr) noexcept {
    coroutine_handle h;
    h.p = __builtin_coro_promise(&pr, alignof(P), true);
    return h;
  }
  P& promise() const { return *static_cast<P*>(__builtin_coro_promise(p, alignof(P), false)); }
  void* address() const noexcept { return p; }
  operator coroutine_handle<>() const noexcept { return coroutine_handle<>::from_address(p); }
  explicit operator bool() const noexcept { return p != nullptr; }
  void resume() const { __builtin_coro_resume(p); }
  void destroy() const { __builtin_coro_destroy(p); }
  bool done() const { return __builtin_coro_done(p); }
};
inline coroutine_handle<> noop_coroutine() noexcept {
  return coroutine_handle<>::from_address(__builtin_coro_noop());
}
struct suspend_always {
  bool await_ready() const noexcept { return false; }
  void await_suspend(coroutine_handle<>) const noexcept {}
  void await_resume() const noexcept {}
};
struct suspend_never {
  bool await_ready() const noexcept { return true; }
  void await_suspend(coroutine_handle<>) const noexcept {}
  void await_resume() const noexcept {}
};
struct nothrow_t {
  explicit nothrow_t() = default;
};
inline constexpr nothrow_t nothrow{};
} // namespace std

extern "C" int printf(const char*, ...);

// A lazy generator of ints: `co_yield` stores the value and suspends.
struct gen {
  struct promise_type {
    int value = 0;
    gen get_return_object() { return gen{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    std::suspend_always yield_value(int v) noexcept {
      value = v;
      return {};
    }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
  gen(std::coroutine_handle<promise_type> c) : h(c) {}
  gen(gen&& o) : h(o.h) { o.h = {}; }
  ~gen() {
    if (h) h.destroy();
  }
  // Resumes; false once the body has finished.
  bool next() {
    h.resume();
    return !h.done();
  }
  int value() const { return h.promise().value; }
};

// An eager task returning an int through `co_return e;`.
struct task {
  struct promise_type {
    int result = -1;
    task get_return_object() { return task{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_value(int v) { result = v; }
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
  task(std::coroutine_handle<promise_type> c) : h(c) {}
  task(task&& o) : h(o.h) { o.h = {}; }
  ~task() {
    if (h) h.destroy();
  }
  int result() const { return h.promise().result; }
};
