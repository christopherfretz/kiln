// [stmt.return.coroutine]/2: `co_return e;` with a non-void operand calls
// `promise.return_value(e)` (a braced-init-list is allowed), `co_return;`
// and `co_return e;` with a void operand call `promise.return_void()`
// (evaluating e first), and flowing off the end of a coroutine whose
// promise has `return_void` is `co_return;` ([stmt.return.coroutine]/3).
// Each then goes to the final suspend point.
//@ expect: run
//@ std: c++20
//@ stdout: value 7
//@ stdout: value 3 4
//@ stdout: side effect
//@ stdout: void
//@ stdout: void
//@ stdout: void
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct pt {
  int x, y;
};

template <class T> struct vjob {
  struct promise_type {
    vjob get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_value(int v) { printf("value %d\n", v); }
    void return_value(pt p) { printf("value %d %d\n", p.x, p.y); }
    void unhandled_exception() {}
  };
};

struct ujob {
  struct promise_type {
    ujob get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() { printf("void\n"); }
    void unhandled_exception() {}
  };
};

vjob<int> val() { co_return 7; }
vjob<pt> braced() { co_return {3, 4}; }
void effect() { printf("side effect\n"); }
ujob void_expr() { co_return effect(); }
ujob plain() { co_return; }
ujob off_end() {
  if (false)
    co_return;
}

int main() {
  val();
  braced();
  void_expr();
  plain();
  off_end();
}
