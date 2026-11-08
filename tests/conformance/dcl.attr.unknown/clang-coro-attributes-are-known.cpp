//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
// clang knows coro_return_type and coro_lifetimebound (clang 18+) and,
// where __has_cpp_attribute says so, coro_await_elidable and
// coro_await_elidable_argument (clang 20+): none of them is unknown.
#include <coroutine>

struct [[clang::coro_return_type, clang::coro_lifetimebound]] Task {
  struct promise_type {
    Task get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

#if __has_cpp_attribute(clang::coro_await_elidable)
struct [[clang::coro_await_elidable]] Elidable {};
#endif
#if __has_cpp_attribute(clang::coro_await_elidable_argument)
void take([[clang::coro_await_elidable_argument]] int x);
#endif

Task run() { co_return; }

int main() { run(); }
