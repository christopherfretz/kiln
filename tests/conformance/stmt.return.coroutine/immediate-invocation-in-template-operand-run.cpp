// [expr.const]/16, [stmt.return.coroutine]: an immediate invocation in
// the operand of `co_return` is a constant expression in an instantiated
// coroutine too; no call to the consteval function is emitted. (Seen as
// `"s2"_id` link errors building folly::coro's AsyncClosureTest.)
//@ expect: run
//@ std: c++20
//@ stdout: 10 7 12
#include <coroutine>
#include <cstdio>

consteval int sq(int x) { return x * x; }
template <int N> consteval int next() { return N + 1; }

struct task {
  struct promise_type {
    int v = 0;
    task get_return_object() {
      return {std::coroutine_handle<promise_type>::from_promise(*this)};
    }
    std::suspend_never initial_suspend() { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_value(int x) { v = x; }
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
};

template <class T> task a(T t) { co_return sq(3) + t; }
template <class T> task b(T t) { co_return next<4>() + t; }
// The non-template form, with `co_return` as the first suspend point.
task c() { co_return sq(3) + 3; }

int main() {
  task x = a(1), y = b(2), z = c();
  std::printf("%d %d %d\n", x.h.promise().v, y.h.promise().v,
              z.h.promise().v);
  x.h.destroy();
  y.h.destroy();
  z.h.destroy();
}
