// [expr.await]/3: the operand is first transformed by
// `promise.await_transform` when the promise declares one; the awaiter is
// then obtained from a member or non-member (ADL) `operator co_await`, or
// the awaitable itself.
//@ expect: run
//@ std: c++20
//@ stdout: member 1
//@ stdout: free 2
//@ stdout: plain 3
//@ stdout: transform 7 -> 70
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

template <class T> struct value_aw {
  T v;
  bool await_ready() { return true; }
  void await_suspend(std::coroutine_handle<>) {}
  T await_resume() { return v; }
};

struct with_member {
  int v;
  value_aw<int> operator co_await() { return {v}; }
};

namespace ns {
struct with_free {
  int v;
};
value_aw<int> operator co_await(with_free w) { return {w.v}; }
} // namespace ns

struct job {
  struct promise_type {
    job get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

job run() {
  printf("member %d\n", co_await with_member{1});
  printf("free %d\n", co_await ns::with_free{2});
  printf("plain %d\n", co_await value_aw<int>{3});
}

struct tjob {
  struct promise_type {
    tjob get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
    value_aw<long> await_transform(int x) {
      printf("transform %d -> ", x);
      return {x * 10L};
    }
  };
};

tjob trun() {
  long r = co_await 7;
  printf("%ld\n", r);
}

int main() {
  run();
  trun();
}
