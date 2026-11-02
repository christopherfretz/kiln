// [dcl.fct.def.coroutine] with clang's `coro_only_destroy_when_complete`
// class attribute: a coroutine whose return type is a class carrying it
// gets the `coro_only_destroy_when_complete` function attribute; one
// returning an unmarked class (or a specialization of an unmarked
// template) does not.
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

template <class Self> struct base {
  std::suspend_always initial_suspend() noexcept { return {}; }
  std::suspend_always final_suspend() noexcept { return {}; }
  void return_void() {}
  void unhandled_exception() {}
};

struct [[clang::coro_only_destroy_when_complete]] marked {
  struct promise_type : base<promise_type> {
    marked get_return_object() { return {std::coroutine_handle<promise_type>::from_promise(*this)}; }
  };
  std::coroutine_handle<promise_type> h;
};
marked m() { co_return; }

template <class T> struct __attribute__((coro_only_destroy_when_complete)) tmarked {
  struct promise_type : base<promise_type> {
    tmarked get_return_object() { return {std::coroutine_handle<promise_type>::from_promise(*this)}; }
  };
  std::coroutine_handle<promise_type> h;
};
tmarked<int> t() { co_return; }

struct plain {
  struct promise_type : base<promise_type> {
    plain get_return_object() { return {std::coroutine_handle<promise_type>::from_promise(*this)}; }
  };
  std::coroutine_handle<promise_type> h;
};
plain p() { co_return; }
