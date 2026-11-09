// `-fsanitize-ignorelist=` on the outlined `await_suspend` wrappers
// ([expr.await]/5.1): clang's `StartFunction` emits each
// `<coroutine>.__await_suspend_wrapper__<kind>` without a declaration, so
// it carries the module's sanitizers less the ignorelist entries that
// name it (`fun:` the wrapper's own name, `src:` and `mainfile:` the main
// file); the `[null]` entry drops the `this` null checks in `g`'s initial
// wrapper only, the final one and `h`'s keep theirs. spec/codegen.md
// "Sanitizers", WP-1043.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=null -fsanitize-ignorelist=%{test_dir}/Inputs/ignorelist-coro.txt
namespace std {
template <class R, class... A> struct coroutine_traits { using promise_type = typename R::promise_type; };
template <class P = void> struct coroutine_handle {
  void *p;
  static coroutine_handle from_address(void *a) noexcept { return {a}; }
  void *address() const noexcept { return p; }
};
} // namespace std

struct promise;
struct task {
  using promise_type = promise;
  void *frame;
};
struct awaiter {
  int n;
  bool await_ready() const noexcept { return false; }
  void await_suspend(std::coroutine_handle<promise>) noexcept { ++n; }
  void await_resume() const noexcept {}
};
struct promise {
  task get_return_object() { return {__builtin_coro_promise(this, alignof(promise), true)}; }
  awaiter initial_suspend() noexcept { return {}; }
  awaiter final_suspend() noexcept { return {}; }
  void return_void() {}
  void unhandled_exception() {}
};

task g() { co_return; }
task h() { co_return; }
