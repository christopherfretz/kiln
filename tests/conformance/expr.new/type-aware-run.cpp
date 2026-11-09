// [expr.new]/20-28, [expr.delete]/9-11, [basic.stc.dynamic] (P2719,
// WP-1088): new- and delete-expressions call the type-aware allocation
// functions with a std::type_identity<T>{} first argument: global and
// class-scope templates, over-aligned and array forms (with the array
// cookie), a virtual destructor's deleting delete, and the matching
// delete a throwing constructor calls, also for a placement form.
//@ expect: run
//@ requires: clang>=22
//@ args: -Wno-ext-cxx-type-aware-allocators
//@ stdout: g new 4 4
//@ stdout: g delete 4 4
//@ stdout: g new 32 32
//@ stdout: g delete 32 32
//@ stdout: g new 32 8
//@ stdout: g delete 32 8
//@ stdout: g new 1 1
//@ stdout: g delete 1 1
//@ stdout: caught
//@ stdout: S new
//@ stdout: S delete
//@ stdout: S new
//@ stdout: S delete
//@ stdout: caught
//@ stdout: S placement new 7
//@ stdout: S placement delete 7
//@ stdout: caught
//@ strict-diags
#include <cstdio>
#include <cstdlib>
#include <new>
#include <type_traits>

template <class T>
void *operator new(std::type_identity<T>, std::size_t n, std::align_val_t a) {
  std::printf("g new %zu %zu\n", n, (std::size_t)a);
  return std::malloc(n);
}
template <class T>
void operator delete(std::type_identity<T>, void *p, std::size_t n, std::align_val_t a) {
  std::printf("g delete %zu %zu\n", n, (std::size_t)a);
  std::free(p);
}

struct alignas(32) O { int x; };
struct P { long y[3]; ~P() {} };
struct V { virtual ~V() {} };
struct W : V { long z[3]; };
struct Th { Th() { throw 1; } };

struct S {
  int x;
  S(bool t) { if (t) throw 1; }
  template <class T>
  void *operator new(std::type_identity<T>, std::size_t n, std::align_val_t) {
    std::puts("S new");
    return ::operator new(n);
  }
  template <class T>
  void operator delete(std::type_identity<T>, void *p, std::size_t, std::align_val_t) {
    std::puts("S delete");
    ::operator delete(p);
  }
  template <class T>
  void *operator new(std::type_identity<T>, std::size_t n, std::align_val_t, int k) {
    std::printf("S placement new %d\n", k);
    return ::operator new(n);
  }
  template <class T>
  void operator delete(std::type_identity<T>, void *p, std::size_t, std::align_val_t, int k) {
    std::printf("S placement delete %d\n", k);
    ::operator delete(p);
  }
};

int main() {
  int *i = new int;
  delete i;
  O *o = new O;
  delete o;
  P *p = new P[1];
  delete[] p;
  V *v = new W;
  delete v;
  try {
    new Th;
  } catch (int) {
    std::puts("caught");
  }
  S *s = new S(false);
  delete s;
  try {
    new S(true);
  } catch (int) {
    std::puts("caught");
  }
  try {
    new (7) S(true);
  } catch (int) {
    std::puts("caught");
  }
}
