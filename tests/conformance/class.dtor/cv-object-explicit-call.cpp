// [class.dtor]/5: a destructor can be invoked for a const, volatile or
// const volatile object, so an explicit destructor call through a
// pointer or reference to a cv-qualified class is well-formed: the
// implicit object parameter's cv-qualifier check ([over.match.funcs]/4)
// does not apply (libc++ `std::__destroy_at<const T>` from
// `ranges::uninitialized_default_construct` over `const T*`). The check
// for other members: class.mem/const-member.cpp. Verified against clang 19.
//@ expect: run
//@ strict-diags
#include <new>
static int dtors = 0;
struct C {
  int v;
  ~C() { ++dtors; }
};
template <class T> void destroy(T* p) { p->~T(); }
struct K {
  constexpr ~K() {}
};
struct U {
  union { K k; };
  constexpr U() : k() {}
  constexpr ~U() {}
};
constexpr bool constant() {
  U u;
  const K* p = &u.k;
  p->~K();
  return true;
}
static_assert(constant());
int main() {
  alignas(C) unsigned char buf[sizeof(C)];
  const C* p = new (buf) const C{1};
  p->~C();
  volatile C* q = new (buf) C{2};
  q->~C();
  const volatile C& r = *new (buf) C{3};
  r.~C();
  destroy(static_cast<const C*>(new (buf) C{4}));
  return dtors == 4 ? 0 : 1;
}
