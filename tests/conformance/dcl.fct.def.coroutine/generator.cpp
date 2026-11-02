// [dcl.fct.def.coroutine]/1, /5, [expr.yield]/1: a function whose body
// holds `co_yield` is a coroutine; the promise comes from
// `std::coroutine_traits<R, P...>::promise_type`; the body is wrapped in
// `initial_suspend`/`final_suspend` and resumed through its handle.
// Free functions, function templates and lambdas.
//@ expect: run
//@ std: c++20
//@ stdout: 0 1 2
//@ stdout: t 0 2 4 6
//@ stdout: l 10 11
//@ exit: 3
#include "Inputs/coro.h"

gen count(int n) {
  for (int i = 0; i < n; ++i) co_yield i;
}

template <class T> gen doubled(T n) {
  for (T i = 0; i < n; ++i) co_yield i * 2;
  co_return;
}

int main() {
  int s = 0;
  {
    gen g = count(3);
    while (g.next()) {
      s += g.value();
      printf(s == 0 ? "%d" : " %d", g.value());
    }
    printf("\n");
  }
  {
    gen g = doubled(4L);
    printf("t");
    while (g.next()) printf(" %d", g.value());
    printf("\n");
  }
  auto l = [](int x) -> gen {
    co_yield x;
    co_yield x + 1;
  };
  gen g = l(10);
  printf("l");
  while (g.next()) printf(" %d", g.value());
  printf("\n");
  return s;
}
