// [dcl.fct.def.coroutine]/6, [dcl.contract.func] (P2900R14): a coroutine
// may have function contract specifiers, but a postcondition that
// odr-uses a non-reference parameter is ill-formed (the parameter has
// been copied into the coroutine frame). A reference parameter, a
// precondition and a non-coroutine are fine; the rule also holds when
// the definition omits the seq of its first declaration. GCC 16.1
// accepts all of it (spec/contracts.md §9), so there is no oracle.
//@ expect: compile-fail
//@ strict-diags
//@ skip(clang): clang 22 does not implement P2900 contracts
//@ diag: 23:19 error "a postcondition of a coroutine cannot odr-use the non-reference parameter 'x'"
//@ diag: 26:18 error "a postcondition of a coroutine cannot odr-use the non-reference parameter 'x'"
#include <coroutine>
struct task {
  struct promise_type {
    task get_return_object() { return {}; }
    std::suspend_never initial_suspend() noexcept { return {}; }
    std::suspend_never final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() {}
  };
};
task co(const int x) pre(x > 0) post(x > 0) { co_return; }
task ok(const int& x, const int y) pre(y > 0) post(x > 0) { co_return; }
task d(const int x) post(x > 0);
task d(const int x) { co_return; }
task notco(const int x) post(x > 0);
