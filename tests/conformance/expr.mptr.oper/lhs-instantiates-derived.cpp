// [expr.mptr.oper]/2-3, [temp.inst]/2: checking that the left operand's
// class is derived from the member pointer's class needs that class
// complete, so `.*` instantiates a class template specialization named
// only by a reference parameter. The access-through-a-private-member
// idiom: an explicit instantiation passes `&B<void>::p` as a template
// argument, and the friend defined there writes through it.
//@ expect: run
//@ stdout: 1 3
#include <cstdio>
template <class T> struct WB { T* p; int n; };
template <class T> struct W : WB<T> {};
void set(W<void>& w, void* ptr);
template <class Tag, void* WB<void>::* F>
struct Acc {
  friend void set(W<void>& w, void* ptr) { w.*F = ptr; }
};
struct Tag {};
template struct Acc<Tag, &WB<void>::p>;
template <int WB<int>::* G> int get(W<int>* w) { return w->*G; }
int main() {
  W<void> w;
  int x;
  set(w, &x);
  W<int> wi;
  wi.n = 3;
  std::printf("%d %d\n", w.p == &x, get<&WB<int>::n>(&wi));
}
