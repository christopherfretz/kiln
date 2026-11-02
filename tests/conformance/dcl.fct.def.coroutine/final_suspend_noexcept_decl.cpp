// [dcl.fct.def.coroutine]/15: co_await promise.final_suspend() shall not
// be potentially-throwing. The note on a function declared without a
// body points at the end of its declarator (clang `getEndLoc`): its last
// token, including a cv-qualifier, the exception specification, a
// trailing return type or requires-clause, an asm label, GNU attributes
// and virt-specifiers, but not C++11 attributes. Members of a class
// template keep their pattern's position.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:27 warning "attribute 'cold' ignored, because it cannot be applied to a type"
//@ diag: 69:25 error "the expression 'co_await __promise.final_suspend()' is required to be non-throwing"
//@ diag: 70:17 note "in instantiation of function template specialization 'f<0>' requested here"
//@ diag: 28:22 note "must be declared with 'noexcept'"
//@ diag: 32:45 note "must be declared with 'noexcept'"
//@ diag: 69:25 error "the expression 'co_await __promise.final_suspend()' is required to be non-throwing"
//@ diag: 71:17 note "in instantiation of function template specialization 'f<1>' requested here"
//@ diag: 38:23 note "must be declared with 'noexcept'"
//@ diag: 69:25 error "the expression 'co_await __promise.final_suspend()' is required to be non-throwing"
//@ diag: 72:17 note "in instantiation of function template specialization 'f<2>' requested here"
//@ diag: 64:34 note "must be declared with 'noexcept'"
//@ diag: 69:25 error "the expression 'co_await __promise.final_suspend()' is required to be non-throwing"
//@ diag: 73:17 note "in instantiation of function template specialization 'f<3>' requested here"
//@ diag: 48:31 note "must be declared with 'noexcept'"
#include "../dcl.fct.def.coroutine/Inputs/coro.h"

struct base { virtual void await_resume(); };
template <int N> struct aw : base {
  bool await_ready() const
    ;
  void await_suspend(std::coroutine_handle<>) noexcept;
  void await_resume() noexcept override;
  ~aw() noexcept(false) __attribute__((cold));
};
template <> struct aw<1> {
  bool await_ready() noexcept;
  void await_suspend(std::coroutine_handle<>) noexcept;
  auto await_resume() noexcept -> int;
  ~aw() noexcept(false) [[gnu::cold]];
};
template <> struct aw<2> {
  bool await_ready() noexcept;
  void await_suspend(std::coroutine_handle<>) noexcept;
  void await_resume() noexcept;
};
template <> struct aw<3> : base {
  bool await_ready() noexcept;
  void await_suspend(std::coroutine_handle<>) noexcept;
  auto await_resume() -> void override
    ;
};

template <int N> struct job {
  struct promise_type {
    job get_return_object();
    std::suspend_never initial_suspend() noexcept;
    aw<N> final_suspend() noexcept;
    void return_void();
    void unhandled_exception();
  };
};
template <> struct job<2>::promise_type {
  job<2> get_return_object();
  std::suspend_never initial_suspend() noexcept;
  aw<2> final_suspend() asm("fs2");
  void return_void();
  void unhandled_exception();
};

template <int N> job<N> f() { co_return; }
template job<0> f<0>();
template job<1> f<1>();
template job<2> f<2>();
template job<3> f<3>();
