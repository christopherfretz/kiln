// [dcl.fct.def.coroutine]/9,12 with -fcoro-aligned-allocation (P2014R0
// option 2, as clang): the coroutine state of an `alignas(64)` promise
// is obtained from `operator new(size_t, align_val_t)` with the frame's
// alignment and freed by the matching aligned `operator delete`, so the
// frame and the promise in it are 64-byte aligned.
//@ expect: run
//@ std: c++20
//@ args: -fcoro-aligned-allocation
//@ stdout: new align 64
//@ stdout: frame 0 promise 0
//@ stdout: delete align 64
//@ stdout: global frame 0 promise 0
#include "Inputs/coro.h"
namespace std { enum class align_val_t : decltype(sizeof 0) {}; }

unsigned long rem(void *p) { return (unsigned long)p % 64; }

template <class Task> struct base {
  struct alignas(64) promise_type {
    char tag;
    Task get_return_object() {
      return Task{std::coroutine_handle<promise_type>::from_promise(*this)};
    }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

struct own_task;
struct own_promise : base<own_task>::promise_type {
  static void *operator new(decltype(sizeof 0) n, std::align_val_t a) {
    printf("new align %lu\n", (unsigned long)a);
    return ::operator new(n, a);
  }
  static void operator delete(void *p, decltype(sizeof 0) n, std::align_val_t a) {
    printf("delete align %lu\n", (unsigned long)a);
    ::operator delete(p, n, a);
  }
};
struct own_task {
  using promise_type = own_promise;
  std::coroutine_handle<> h;
  explicit own_task(std::coroutine_handle<base<own_task>::promise_type> c)
      : h(std::coroutine_handle<>::from_address(c.address())) {}
};

struct global_task {
  using promise_type = base<global_task>::promise_type;
  std::coroutine_handle<promise_type> h;
};

own_task f() { co_return; }
global_task g() { co_return; }

int main() {
  own_task t = f();
  auto &p = std::coroutine_handle<own_promise>::from_address(t.h.address()).promise();
  printf("frame %lu promise %lu\n", rem(t.h.address()), rem(&p));
  t.h.destroy();
  global_task u = g();
  printf("global frame %lu promise %lu\n", rem(u.h.address()), rem(&u.h.promise()));
  u.h.destroy();
}
