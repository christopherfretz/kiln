// [expr.await]/5.7: a co_await (and a co_yield) whose `await_resume`
// returns a `_Complex` prvalue is that complex value, emitted as its two
// parts (WP-1739: rejected by code generation; clang
// `ComplexExprEmitter::VisitCoawaitExpr`). Its full-expression's
// cleanups are shared with the suspension's destroy path, so the parts
// are carried across them (clang `ForceCleanup`).
//@ expect: ir
//@ std: c++20
//@ min-oracle: 22
#include <coroutine>
struct Aw {
  bool r;
  bool await_ready() { return r; }
  void await_suspend(std::coroutine_handle<>) {}
  _Complex double await_resume() { return 5.0 + 6.0i; }
};
struct Task {
  struct promise_type {
    _Complex double v;
    Task get_return_object() { return {}; }
    std::suspend_never initial_suspend() { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
    Aw yield_value(_Complex double y) { v = y; return {false}; }
  };
};
_Complex double out, x = 1.0 + 2.0i;
Task co(bool b) {
  _Complex double a = co_await Aw{b};
  out = a + co_await Aw{!b};
  co_await Aw{b};
  out = co_yield x;
}
