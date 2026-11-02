// [dcl.fct.def.coroutine]/5, /9-/11: the promise is constructed from the
// parameters when such a constructor is viable; the coroutine state is
// allocated by the promise's `operator new` (with the parameters appended
// when that form is viable), freed by its `operator delete`; with
// `get_return_object_on_allocation_failure` the allocation function is
// non-throwing and a null result returns that object.
//@ expect: run
//@ std: c++20
//@ stdout: new args 2 3
//@ stdout: promise 2 3
//@ stdout: r 5
//@ stdout: delete
//@ stdout: fail -1
#include "Inputs/coro.h"

void* operator new(decltype(sizeof 0), const std::nothrow_t&) noexcept;
void* operator new(decltype(sizeof 0));
void operator delete(void*) noexcept;

static bool fail_next = false;

struct sum {
  struct promise_type {
    int result = 0;
    promise_type(int a, int b) { printf("promise %d %d\n", a, b); }
    promise_type() {}
    static void* operator new(decltype(sizeof 0) n, int a, int b) {
      printf("new args %d %d\n", a, b);
      return ::operator new(n);
    }
    static void operator delete(void* p) {
      printf("delete\n");
      ::operator delete(p);
    }
    sum get_return_object() { return sum{std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_value(int v) { result = v; }
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
};

sum add(int a, int b) { co_return a + b; }

struct maybe {
  struct promise_type {
    static void* operator new(decltype(sizeof 0) n) noexcept {
      return fail_next ? nullptr : ::operator new(n, std::nothrow);
    }
    static maybe get_return_object_on_allocation_failure() { return maybe{-1}; }
    maybe get_return_object() { return maybe{1}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  int state;
};

maybe try_run() { co_return; }

int main() {
  sum s = add(2, 3);
  printf("r %d\n", s.h.promise().result);
  s.h.destroy();
  fail_next = true;
  printf("fail %d\n", try_run().state);
}
