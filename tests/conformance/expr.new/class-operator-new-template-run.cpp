// [expr.new]/19-20, [temp.over]/1: a class-scope allocation function
// template is a candidate; its specialization is deduced from the size
// and the placement arguments, instantiated and called (abseil's
// exception-safety testing types forward placement new this way).
//@ expect: run
//@ stdout: 7 1 2 9
#include <cstddef>
#include <cstdio>
#include <new>
#include <utility>

static int calls = 0;

struct T {
  int v;
  T(int x) : v(x) {}
  template <class... Args>
  static void *operator new(std::size_t s, Args &&...args) noexcept {
    ++calls;
    return ::operator new(s, std::forward<Args>(args)...);
  }
  static void operator delete(void *, void *) noexcept {}
  static void operator delete(void *p) noexcept { ::operator delete(p); }
};

template <class U> struct W {
  U u;
  W(U x) : u(x) {}
  template <class P> static void *operator new(std::size_t, P *where) {
    ++calls;
    return where;
  }
};

int main() {
  alignas(T) unsigned char buf[sizeof(T)];
  T *p = new (&buf) T(7);
  int a = p->v;
  int b = static_cast<void *>(p) == static_cast<void *>(buf);
  alignas(W<int>) unsigned char wbuf[sizeof(W<int>)];
  W<int> *w = new (wbuf) W<int>(9);
  std::printf("%d %d %d %d\n", a, b, calls, w->u);
}
