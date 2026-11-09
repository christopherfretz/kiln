// [expr.prim.id.dtor]/2, [basic.lookup.qual.general]/4, [expr.ref]/1,
// [dcl.type.simple]/2: a pseudo-destructor-name whose type-names are
// simple-template-ids, `p->~A<int>()`, `p->A<int>::~A<int>()`,
// `p->N::I::~B<int>()` (`B` looked up in the prefix `N`, 4.2), and the
// dependent `p->~A<T>()`, `p->template A<T>::~A<T>()`,
// `p->::N::template B<T>::~B<T>()` instantiated with `T = int`: the call
// does nothing but still evaluates the object expression. With `T` a
// class, or `p->~CA<int>()` through an alias template of a class
// template, it calls the class's destructor.
//@ expect: run
//@ strict-diags
//@ stdout: calls=12
//@ stdout: ~C
//@ stdout: ~C
//@ stdout: ~C
extern "C" int puts(const char*);
extern "C" int printf(const char*, ...);
inline void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
template <class T> using A = T;
namespace N {
using I = int;
template <class T> using B = T;
template <class X> struct C { ~C() { puts("~C"); } };
}
template <class X> using CA = N::C<X>;
using I2 = int;
int calls;
template <class T> T* obj(T* p) { ++calls; return p; }
void scalar(int* p) {
  obj(p)->~A<int>();
  obj(p)->A<int>::~A<int>();
  obj(p)->N::I::~B<int>();
  obj(p)->::N::I::~B<int>();
  obj(p)->N::B<int>::~B<int>();
  obj(p)->I2::~A<int>();
  obj(p)->::N::I::~I();
  (*obj(p)).~A<int>();
}
template <class T> void dependent(T* p) {
  obj(p)->~A<T>();
  obj(p)->template A<T>::~A<T>();
  obj(p)->::N::template B<T>::~B<T>();
}
template <class X> void cls(N::C<X>* p) { p->~CA<X>(); }
template <class T> void alias_cls(T* p) { p->~A<T>(); }
int main() {
  int i = 1;
  scalar(&i);
  dependent(&i);
  // Ends `i`'s lifetime: recreate it for the next call.
  ::new (&i) int(2);
  obj(&i)->~A<int>();
  ::new (&i) int(3);
  printf("calls=%d\n", calls);
  alignas(N::C<int>) unsigned char buf[sizeof(N::C<int>)];
  N::C<int>* c = ::new (buf) N::C<int>;
  c->~CA<int>();
  c = ::new (buf) N::C<int>;
  cls(c);
  c = ::new (buf) N::C<int>;
  alias_cls(c);
  return 0;
}
