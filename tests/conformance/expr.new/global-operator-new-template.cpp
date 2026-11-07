// [expr.new]/19, [temp.over]: a global allocation function template is a
// candidate of a new-expression's lookup in the global scope, its
// specialization deduced from the placement arguments (LLVM's `template
// <...> void *operator new(size_t, BumpPtrAllocatorImpl<...> &)` in
// Allocator.h, called as `new (getAlloc()) Node(...)` from a class
// template). A class's own `operator new` still hides it.
//@ expect: run
//@ strict-diags
//@ std: c++17
//@ stdout: 3 5 7 c1 2
#include <cstddef>
#include <cstdio>
namespace ns {
template <class A, std::size_t N = 64> struct Arena {
  alignas(std::max_align_t) char buf[N];
  std::size_t used = 0;
  void *get(std::size_t s) {
    void *p = buf + used;
    used += (s + 15) / 16 * 16;
    return p;
  }
};
} // namespace ns
template <class A, std::size_t N>
void *operator new(std::size_t s, ns::Arena<A, N> &a) { return a.get(s); }
template <class A, std::size_t N>
void operator delete(void *, ns::Arena<A, N> &) {}
template <class T>
void *operator new[](std::size_t s, T &a, int) { return a.get(s); }
static int class_new = 0;
struct X {
  int v;
  X(int v) : v(v) {}
};
struct Own {
  int v;
  static void *operator new(std::size_t s, ns::Arena<int> &a) {
    ++class_new;
    return a.get(s);
  }
};
template <class T, class Alloc> struct List : Alloc {
  Alloc &getAlloc() { return *this; }
  template <class... Ts> T *create(Ts &&...ts) { return new (getAlloc()) T(ts...); }
};
int main() {
  ns::Arena<int> a;
  X *p = new (a) X(3);
  List<X, ns::Arena<char, 128>> l;
  X *q = l.create(5);
  int *r = new (a, 0) int[2]{7, 8};
  Own *o = new (a) Own{1};
  std::printf("%d %d %d c%d %d\n", p->v, q->v, r[0], class_new, (int)(a.used / 16) - 1);
  (void)o;
}
