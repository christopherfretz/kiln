// [dcl.fct.def.coroutine]/15: co_await promise.final_suspend() shall not
// be potentially-throwing. A namespace-scope function declared without a
// body (here a non-member operator co_await) is noted at the end of its
// declarator, as a member is (final_suspend_noexcept_decl.cpp): the
// exception specification, GNU attributes but not a C++11 attribute run,
// an asm label, a trailing return type and requires-clause. Other notes
// on such a declaration (a candidate function) stay at its name.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 47:11 error "the expression 'co_await __promise.final_suspend()' is required to be non-throwing"
//@ diag: 32:42 note "must be declared with 'noexcept'"
//@ diag: 48:11 error "the expression 'co_await __promise.final_suspend()' is required to be non-throwing"
//@ diag: 33:53 note "must be declared with 'noexcept'"
//@ diag: 49:11 error "the expression 'co_await __promise.final_suspend()' is required to be non-throwing"
//@ diag: 34:47 note "must be declared with 'noexcept'"
//@ diag: 50:11 error "the expression 'co_await __promise.final_suspend()' is required to be non-throwing"
//@ diag: 35:76 note "must be declared with 'noexcept'"
//@ diag: 53:14 error "no matching function for call to 'g'"
//@ diag: 52:6 note "candidate function not viable: no known conversion from 'const char[2]' to 'int' for 1st argument"
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct fin0 {};
struct fin1 {};
struct fin2 {};
struct fin3 {};
struct aw {
  bool await_ready() noexcept;
  void await_suspend(std::coroutine_handle<>) noexcept;
  void await_resume() noexcept;
};

aw operator co_await(fin0) noexcept(false);
aw operator co_await(fin1) [[]] __attribute__((cold));
aw operator co_await(fin2) asm("co_await_fin2");
template <class T> auto operator co_await(T) -> aw requires (sizeof(T) == 1);

template <class F> struct job {
  struct promise_type {
    job get_return_object();
    std::suspend_never initial_suspend() noexcept;
    F final_suspend() noexcept;
    void return_void();
    void unhandled_exception();
  };
};

job<fin0> f0() { co_return; }
job<fin1> f1() { co_return; }
job<fin2> f2() { co_return; }
job<fin3> f3() { co_return; }

void g(int) noexcept(true) __attribute__((cold));
void use() { g("x"); }
