// [expr.await]/3.3, [temp.dep.candidate]: in an instantiated coroutine,
// a non-member `operator co_await` that is not viable for an awaiter
// (here `operator co_await(F)` and the promise's `std::suspend_never`)
// leaves the awaiter as the operand; kiln reported the function as not
// found by two-phase lookup (clang `DiagnoseTwoPhaseLookup` names only a
// viable one). Member and non-member `operator co_await` calls are
// formed again for the substituted operand. WP-1738 (GAP-054).
//@ expect: run
//@ stdout: 10 21 8
#include <coroutine>
#include <cstdio>
struct Task {
  struct promise_type {
    Task get_return_object() { return {}; }
    std::suspend_never initial_suspend() { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};
struct Aw {
  int v;
  bool await_ready() { return true; }
  void await_suspend(std::coroutine_handle<>) {}
  int await_resume() { return v; }
};
struct M { int v; Aw operator co_await() const { return {v * 2}; } };
struct F { int v; };
Aw operator co_await(F f) { return {f.v * 3}; }
template <class T> Task run(T seed) {
  M m{5};
  F f{7};
  int a = co_await m;
  int b = co_await f;
  T t = seed;
  int c = co_await M{int(t)};
  std::printf("%d %d %d\n", a, b, c);
  co_return;
}
int main() { run(4); }
