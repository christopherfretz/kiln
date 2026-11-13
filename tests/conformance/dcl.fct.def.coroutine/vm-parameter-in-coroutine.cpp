// [dcl.fct.def.coroutine]/4, [temp.arg.type] (the variable length array
// extension): the promise type of a coroutine is looked up in
// `std::coroutine_traits<R, P1, ..., Pn>` with the parameter types as
// template arguments, and a variably modified type cannot be one (clang
// `err_variably_modified_template_arg`, at the first coroutine keyword),
// whether written as a pointer to a variable length array or as an array
// parameter adjusted to one, and in a member function. An ordinary
// function may take such a parameter, and a coroutine may take a plain
// pointer. WP-1386.
//@ expect: compile-fail
//@ std: c++20
//@ args: -Wno-vla-cxx-extension
//@ strict-diags
//@ diag: 29:31 error "variably modified type 'int (*)[n]' cannot be used as a template argument"
//@ diag: 30:30 error "variably modified type 'int (*)[n]' cannot be used as a template argument"
//@ diag: 31:53 error "variably modified type 'char (*)[m][n]' cannot be used as a template argument"
#include "Inputs/coro.h"

struct vtask {
  struct promise_type {
    vtask get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};

vtask f(int n, int (*p)[n]) { co_return; }
vtask g(int n, int a[][n]) { co_return; }
struct S { vtask m(int m, int n, char (*p)[m][n]) { co_return; } };
vtask ok(int n, int *p) { co_return; }
int plain(int n, int (*p)[n]) { return sizeof *p; }
