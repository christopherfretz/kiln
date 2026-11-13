// [dcl.fct.def.coroutine]/13, [expr.call]/7 (a parameter of a class with
// the trivial_abi attribute is destroyed by the callee): a coroutine's
// own parameter objects belong to the ramp, never to the coroutine frame.
// Each coroutine here runs to completion inside its ramp, so the frame is
// freed before the ramp returns and destroys its parameter; the parameter
// must not live in the frame (kiln marks its alloca
// `!coro.outside.frame`, as clang 22 does; clang 19 reads freed memory).
// The malloc tunables fill freed memory with 0x5a so a read of it shows.
// Also a member and a lambda coroutine, and a parameter whose copy throws
// (the ramp frees the frame, then destroys the parameter). WP-1374.
//@ expect: run
//@ std: c++20
//@ requires: clang>=22
//@ oracle: none
//@ env: GLIBC_TUNABLES=glibc.malloc.tcache_count=0
//@ env: MALLOC_PERTURB_=165
//@ stdout: body 7
//@ stdout: ~TA 7
//@ stdout: ~TA -1
//@ stdout: mem 9
//@ stdout: ~TA 8
//@ stdout: ~TA -1
//@ stdout: lam 9
//@ stdout: ~TA 9
//@ stdout: ~TA -1
//@ stdout: ~TA 99
//@ stdout: caught 99
//@ stdout: tmpl 5
//@ stdout: ~TA 5
//@ stdout: ~TA -1
//@ stdout: ~TA -1
//@ stdout: later 6
//@ stdout: ~TA 6
#include "Inputs/coro.h"
extern "C" int printf(const char*, ...);

struct [[clang::trivial_abi]] TA {
  long x;
  TA(long v) : x(v) {}
  TA(TA&& o) : x(o.x) {
    if (o.x == 99) throw 99;
    o.x = -1;
  }
  ~TA() { printf("~TA %ld\n", x); }
};

struct sync {
  struct promise_type {
    sync get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

sync f(TA t) {
  printf("body %ld\n", t.x);
  co_return;
}

struct S {
  long m;
  sync mem(TA t) {
    printf("mem %ld\n", t.x + m);
    co_return;
  }
};

auto lam = [](TA t) -> sync {
  printf("lam %ld\n", t.x);
  co_return;
};

template <class T>
sync tf(T t) {
  printf("tmpl %ld\n", t.x);
  co_return;
}

// Suspended when the ramp returns: the body runs later from the frame's
// copy, after the ramp has destroyed its own parameter.
struct lazy {
  struct promise_type {
    lazy get_return_object() {
      return {std::coroutine_handle<promise_type>::from_promise(*this)};
    }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
  std::coroutine_handle<promise_type> h;
};

template <class T>
lazy later(T t) {
  printf("later %ld\n", t.x);
  co_return;
}

int main() {
  f(TA(7));
  S{1}.mem(TA(8));
  lam(TA(9));
  try {
    f(TA(99));
  } catch (int e) {
    printf("caught %d\n", e);
  }
  tf(TA(5));
  lazy l = later(TA(6));
  l.h.resume();
  l.h.destroy();
}
