// [basic.link]/6: a block-scope function or `extern` variable
// declaration in an instantiation of a function template names the
// namespace-scope entity (declared before or after it), not a local of
// the instantiation; linking fails if it mangled as a local name.
//@ expect: run
//@ stdout: 11 12 12 14 1100 1
#include <cstdio>
int g(int x);
template <class T> int f(T t) { extern int g(int); extern int ev; return g(ev) + (int)t; }
template <class T> int f2(T) { extern int h(T); extern int ev; return h(0) + ev; }
namespace N {
template <class T> int f3() { extern int k(); extern int nv; return k() + nv; }
int k() { return 100; }
int nv = 1000;
}
void plain() { extern int ev; ev += 0; }
int ev = 5;
template <class T> bool same() { extern int g(int); extern int ev; return &g == &::g && &ev == &::ev; }
int g(int x) { return x * 2; }
int h(int x) { return x + 7; }
int h(char x) { return x + 9; }
int main() {
  plain();
  std::printf("%d %d %d %d %d %d\n", f(1), f(2L), f2(1), f2(char(0)), N::f3<int>(), (int)same<int>());
}
