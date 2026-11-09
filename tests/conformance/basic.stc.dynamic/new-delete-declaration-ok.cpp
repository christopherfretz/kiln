// [basic.stc.dynamic.allocation]/1, [basic.stc.dynamic.deallocation]/1-2
// valid neighbours (WP-1015): replacement and placement global forms,
// nothrow, aligned, sized and variadic forms, cv-qualified parameters,
// class-specific templates, a destroying operator delete, a block-scope
// redeclaration, and a class template's members.
//@ std: c++26
//@ expect: compile-ok
#include <new>
#include <memory>
#include <cstdlib>
void* operator new(std::size_t n) { return std::malloc(n ? n : 1); }
void operator delete(void* p) noexcept { std::free(p); }
void operator delete(void* p, std::size_t) noexcept { std::free(p); }
void* operator new[](std::size_t n, const std::nothrow_t&) noexcept { return std::malloc(n); }
void* operator new(std::size_t, double, ...);
void operator delete(void*, double);
inline void* operator new(std::size_t, void* p, int) noexcept { return p; }
struct A {
  static void* operator new(std::size_t n) { return ::operator new(n); }
  void* operator new(std::size_t n, std::align_val_t a) { return ::operator new(n, a); }
  void* operator new[](const std::size_t n) { return ::operator new(n); }
  void* operator new(std::size_t, void* p) { return p; }
  void operator delete(void* p) { ::operator delete(p); }
  void operator delete[](void* const p, std::size_t) { ::operator delete(p); }
  void operator delete(void*, std::align_val_t) {}
  template<class T> void* operator new(std::size_t n, T) { return ::operator new(n); }
  template<class T> void operator delete(void*, T) {}
  template<class... T> void* operator new(std::size_t, T...);
};
struct D { void operator delete(D* p, std::destroying_delete_t) { ::operator delete(p); } };
template<class T> struct B {
  void* operator new(std::size_t n) { return ::operator new(n); }
  void operator delete(void* p) { ::operator delete(p); }
  void* operator new(std::size_t n, T*) { return ::operator new(n); }
  void operator delete(void* p, T) {}
  void operator delete(B* p, std::destroying_delete_t) { ::operator delete(p); }
};
using sz = decltype(sizeof 0);
typedef void* vp;
struct E { vp operator new(sz); void operator delete(vp) noexcept; };
auto *a = new A; auto *b = new B<int>; auto *dd = new D;
void f() {
  void* operator new(std::size_t);
  delete a; delete b; delete dd;
  std::unique_ptr<int> u(new int);
}
int main() {}
