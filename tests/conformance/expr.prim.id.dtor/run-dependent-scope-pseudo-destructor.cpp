// [basic.lookup.qual.general]/2-4, [expr.prim.id.dtor]/2,
// [temp.res.general]/1: in `p->T::Inner::~Inner()` on a dependent object
// `T` is a member-qualified name looked up at instantiation in the
// object's type and, finding nothing there, by unqualified lookup (2);
// `~Inner` is then looked up in the prefix `T` (4.2), and both type-names
// must name the object's type. kiln follows the standard and GCC 16;
// clang 19 and 22 reject the instantiation ("use of undeclared identifier
// 'T'": no unqualified fallback for a dependent object), a clang defect.
//@ expect: run
//@ strict-diags
//@ oracle: gcc
//@ skip(clang): clang 19/22 reject `p->T::Inner::~Inner()` on a dependent object (WP-1155)
//@ stdout: ~Inner
//@ stdout: ~TI
//@ stdout: calls=3
extern "C" int puts(const char*);
extern "C" int printf(const char*, ...);
inline void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
struct S {
  struct Inner { ~Inner() { puts("~Inner"); } };
  using I = int;
  template <class U> struct TI { ~TI() { puts("~TI"); } };
};
int calls;
template <class T> T* obj(T* p) { ++calls; return p; }
template <class T> void inner(typename T::Inner* p) { obj(p)->T::Inner::~Inner(); }
template <class T> void scalar(typename T::I* p) { obj(p)->T::I::~I(); }
template <class T> void member_template(typename T::template TI<int>* p) {
  obj(p)->T::template TI<int>::~TI();
}
int main() {
  alignas(S::Inner) unsigned char b1[sizeof(S::Inner)];
  inner<S>(::new (b1) S::Inner);
  alignas(S::TI<int>) unsigned char b2[sizeof(S::TI<int>)];
  member_template<S>(::new (b2) S::TI<int>);
  int i = 0;
  scalar<S>(&i);
  printf("calls=%d\n", calls);
}
