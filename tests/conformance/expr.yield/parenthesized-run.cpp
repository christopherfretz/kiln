// [expr.yield]/1, [expr.prim.paren]: a yield-expression is an
// assignment-expression, so it may be parenthesized as an operand:
// `i = (co_yield i) + i` and `42 == (co_yield n)` (Boost.Asio's
// `experimental::coro` tests; kiln parsed the parenthesized form as a fold
// operand and reported "expected expression", WP-812).
//@ expect: run
//@ std: c++20
//@ stdout: 1 3 1
#include <coroutine>
extern "C" int printf(const char*, ...);
struct G {
  struct promise_type {
    int last = 0;
    G get_return_object() { return {}; }
    std::suspend_never initial_suspend() { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void unhandled_exception() {}
    void return_void() {}
    struct A {
      int v;
      bool await_ready() { return true; }
      void await_suspend(std::coroutine_handle<>) {}
      int await_resume() { return v; }
    };
    A yield_value(int x) { return {x + 1}; }
  };
};
G f() {
  int i = 1;
  i = (co_yield i) + i;   // (1 + 1) + 1
  bool b = 4 ==(co_yield i);
  printf("%d %d %d\n", 1, i, int(b));
}
int main() { f(); }
