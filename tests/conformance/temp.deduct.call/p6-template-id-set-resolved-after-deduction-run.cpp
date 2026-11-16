// [temp.deduct.call]/6, [over.over]/3: a function template-id argument is
// an overload set. `foo<int>` names `foo<int>(int*)` alone, but against
// `void (*)(int*, U)` no member of the set deduces, so the argument is a
// non-deduced context: `U` comes from the other argument and the
// conversion then selects `foo<int, int>` (as `&foo<int>`, in
// parentheses, through a reference or function parameter, as a pointer to
// member, with a later defaulted parameter, and among constrained
// templates, CWG2918). Found running GCC's C++ testsuite.
//@ expect: run
//@ strict-diags
//@ std: c++20
//@ stdout: 1 2 3 4 5 6 7 8 9 10
#include <cstdio>
int seen = 0;
template <class T> void foo(T *) { seen = -1; }
template <class T, class U> void foo(T *, U u) { seen = u; }
template <class T, class U> void by_ptr(void (*f)(T, U), U u) { f(nullptr, u); }
template <class T, class U> void by_ref(void (&f)(T, U), U u) { f(nullptr, u); }
template <class T, class U> void by_fn(void f(T, U), U u) { f(nullptr, u); }
template <class T, class U> void defaulted(void (*f)(T, U), U u, int = 0) { f(nullptr, u); }
struct S {
  template <class T> void m(T *) { seen = -1; }
  template <class T, class U> void m(T *, U u) { seen = u; }
};
template <class T, class U> void by_mem(void (S::*f)(T, U), U u) { S s; (s.*f)(nullptr, u); }
template <class T> concept Small = sizeof(T) <= 4;
template <class T, class U> requires Small<U> void con(T *, U u) { seen = u; }
template <class T, class U> void con(T *, U) { seen = -2; }
int main() {
  by_ptr<int *>(foo<int>, 1); std::printf("%d", seen);
  by_ptr<int *>(&foo<int>, 2); std::printf(" %d", seen);
  by_ptr<int *>((foo<int>), 3); std::printf(" %d", seen);
  by_ptr<int *>((&foo<int>), 4); std::printf(" %d", seen);
  by_ref<int *>(foo<int>, 5); std::printf(" %d", seen);
  by_fn<int *>(foo<int>, 6); std::printf(" %d", seen);
  defaulted<int *>(foo<int>, 7); std::printf(" %d", seen);
  by_mem<int *>(&S::m<int>, 8); std::printf(" %d", seen);
  by_ptr<int *>(con<int>, 9); std::printf(" %d", seen);
  by_ptr<int *>(&foo, 10); std::printf(" %d\n", seen);
}
