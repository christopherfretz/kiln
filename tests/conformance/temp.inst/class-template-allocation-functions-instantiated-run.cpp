// [temp.inst]/11, [basic.def.odr]/7: the allocation and deallocation
// functions a new- or delete-expression selects in a class template
// specialization are odr-used there, so they are instantiated (kiln left
// X<int>::operator new and operator delete undefined at link time;
// abseil's exception-safety testing types).
//@ expect: run
//@ stdout: 3 2 2
#include <cstddef>
#include <cstdio>
#include <new>

static int news = 0, dels = 0;

template <class T> struct X {
  T v;
  X(T x) : v(x) {
    if (x < 0)
      throw 1;
  }
  static void *operator new(std::size_t s) {
    ++news;
    return ::operator new(s);
  }
  void operator delete(void *p) noexcept {
    ++dels;
    ::operator delete(p);
  }
};

int main() {
  X<int> *p = new X<int>(3);
  try {
    new X<int>(-1);  // the matching operator delete frees the storage
  } catch (int) {
  }
  int v = p->v;
  delete p;
  std::printf("%d %d %d\n", v, news, dels);
}
