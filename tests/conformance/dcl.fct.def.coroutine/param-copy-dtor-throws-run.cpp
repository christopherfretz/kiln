// [dcl.fct.def.coroutine]/13, [except.ctor]/3: the parameter copies live in
// the coroutine frame and are destroyed with it; when one copy's destructor
// throws during `destroy()`, the copies not yet destroyed still are (here
// `P11` after `Q12` throws). When the promise constructor throws after the
// copies are made, the copies are destroyed in reverse order, then the
// call's argument temporaries, and the exception reaches the caller.
// Found running GCC's C++ testsuite. Matches clang 19 and 22; GCC 16.1
// leaks `P11`.
//@ expect: run
//@ std: c++20
//@ strict-diags
//@ stdout: cp11 cp12 ~Q2 ~P1 | body | destroy ~Q12 ~P11 caught 7|cp13 cp14 promise-throw ~Q14 ~P13 ~Q4 ~P3 caught 5|
#include <coroutine>
extern "C" int printf(const char*, ...);
struct P1 { int i; P1(int i) : i(i) {} P1(const P1& o) : i(o.i + 10) { printf("cp%d ", i); } ~P1() { printf("~P%d ", i); } };
struct P2 { int i; P2(int i) : i(i) {} P2(const P2& o) : i(o.i + 10) { printf("cp%d ", i); }
  ~P2() noexcept(false) { printf("~Q%d ", i); if (i == 12) throw 7; } };
bool promise_throws;
struct Task {
  struct promise_type {
    promise_type() { if (promise_throws) { printf("promise-throw "); throw 5; } }
    Task get_return_object() { return {std::coroutine_handle<promise_type>::from_promise(*this)}; }
    std::suspend_always initial_suspend() noexcept { return {}; }
    std::suspend_always final_suspend() noexcept { return {}; }
    void return_void() {}
    void unhandled_exception() { throw; }
  };
  std::coroutine_handle<promise_type> h;
};
Task co(P1 a, P2 b) { printf("body "); co_return; }
int main() {
  try {
    Task t = co(P1(1), P2(2));
    printf("| ");
    t.h.resume();
    printf("| destroy ");
    t.h.destroy();
  } catch (int e) { printf("caught %d|", e); }
  promise_throws = true;
  try { Task t = co(P1(3), P2(4)); } catch (int e) { printf("caught %d|", e); }
  printf("\n");
}
