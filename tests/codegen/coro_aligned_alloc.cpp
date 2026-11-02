// [dcl.fct.def.coroutine]/9-12 under -fcoro-aligned-allocation (clang's
// P2014R0 option 2): the frame is allocated with
// `operator new(__builtin_coro_size(), std::align_val_t(__builtin_coro_align()))`
// and freed with the sized aligned `operator delete`; a promise's own
// aligned forms are preferred, as is the aligned nothrow form under
// `get_return_object_on_allocation_failure`.
//@ args: -fcoro-aligned-allocation
namespace std {
enum class align_val_t : decltype(sizeof 0) {};
struct nothrow_t {};
extern const nothrow_t nothrow;
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
void *operator new(decltype(sizeof 0), std::align_val_t, const std::nothrow_t &) noexcept;

template <class Self> struct base {
  std::suspend_always initial_suspend() noexcept { return {}; }
  std::suspend_always final_suspend() noexcept { return {}; }
  void return_void() {}
  void unhandled_exception() {}
};

struct global_task {
  struct alignas(64) promise_type : base<promise_type> {
    global_task get_return_object() {
      return {std::coroutine_handle<promise_type>::from_promise(*this)};
    }
  };
  std::coroutine_handle<promise_type> h;
};
global_task g() { co_return; }

struct own_task {
  struct promise_type : base<promise_type> {
    own_task get_return_object() {
      return {std::coroutine_handle<promise_type>::from_promise(*this)};
    }
    static void *operator new(decltype(sizeof 0) n, std::align_val_t a, int);
    static void *operator new(decltype(sizeof 0) n, std::align_val_t a);
    static void *operator new(decltype(sizeof 0) n);
    static void operator delete(void *p, decltype(sizeof 0) n, std::align_val_t a);
  };
  std::coroutine_handle<promise_type> h;
};
own_task o(int) { co_return; }
own_task o2(double) { co_return; }

struct nothrow_task {
  struct promise_type : base<promise_type> {
    nothrow_task get_return_object() {
      return {std::coroutine_handle<promise_type>::from_promise(*this)};
    }
    static nothrow_task get_return_object_on_allocation_failure() { return {}; }
  };
  std::coroutine_handle<promise_type> h;
};
nothrow_task n() { co_return; }
