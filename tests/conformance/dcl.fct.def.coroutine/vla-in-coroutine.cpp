// [dcl.fct.def.coroutine], [dcl.array] (the variable length array
// extension): a coroutine's frame has a fixed size, so a variable length
// array type written in a coroutine body is an error, once per coroutine
// at the first (clang `err_vla_in_coroutine_unsupported`, also for a
// pointer to one, a typedef and `sizeof`, and with a statement
// expression). A nested lambda, a local class's member function and an
// ordinary function may use one. WP-1374.
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-vla-cxx-extension
//@ strict-diags
//@ diag: 32:23 error "variable length arrays in a coroutine are not supported"
//@ diag: 33:43 error "variable length arrays in a coroutine are not supported"
//@ diag: 34:26 error "variable length arrays in a coroutine are not supported"
//@ diag: 35:37 error "variable length arrays in a coroutine are not supported"
//@ diag: 36:34 error "variable length arrays in a coroutine are not supported"
//@ diag: 37:23 error "variable length arrays in a coroutine are not supported"
#include "Inputs/coro.h"
void use(int*);

struct vgen {
  struct promise_type {
    vgen get_return_object() { return {}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    std::suspend_always yield_value(int) noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

vgen v1(int n) { int a[n]; use(a); co_yield 1; int b[n]; use(b); }
vgen v2(int n) { co_yield 1; typedef int T[n]; T *p = nullptr; (void)p; }
vgen v3(int n) { int (*q)[n] = nullptr; (void)q; co_return; }
vgen v4(int n) { co_yield sizeof(int[n]); }
vgen v5(int n) { int x = ({ int z[n]; z[0] = 1; z[0]; }); co_yield x; }
vgen v6(int n) { int a[n]; use(a); co_await std::suspend_always{}; }

vgen l1(int n) { auto l = [n] { int c[n]; use(c); }; l(); co_yield 1; }
vgen l2(int n) { struct L { static void f(int m) { int e[m]; use(e); } }; L::f(n); co_yield 1; }
void plain(int n) { int d[n]; use(d); }
