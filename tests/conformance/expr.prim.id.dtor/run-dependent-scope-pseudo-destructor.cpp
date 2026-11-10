// [basic.lookup.qual.general]/2-4, [temp.dep.type]/5.2, [expr.prim.id.dtor]/2,
// [temp.res.general]/1: in `p->T::Inner::~Inner()` on a dependent object
// `T` is a member-qualified name (/2) whose lookup context, the object's
// type, is dependent, so `T` is a dependent name looked up for each
// specialization; there, no name is dependent (/3, note 2): `T` is looked
// up in the object's type and, finding nothing, by unqualified lookup (/3,
// last sentence; the same rule makes `p->T::~T()` work). `~Inner` is then
// looked up in the prefix `T` (4.2) and must name `T::Inner` (4.6). The
// namespace-qualified `p->M::K::~K()` / `p->M::J::~J()` on a dependent
// object are the same rule with `M` a namespace. kiln follows the standard
// and GCC 16. clang 19 and 22 reject each at instantiation ("use of
// undeclared identifier 'T'"/"'M'"), though they accept the same
// qualifier in `p->T::Inner::f()` and the non-dependent forms: a clang
// defect (WP-1155, WP-1173; spec/sema-class.md).
//@ expect: run
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): clang 19/22 do not look up the first qualifier of a destructor name unqualified at instantiation (WP-1173)
//@ stdout: ~Inner
//@ stdout: ~TI
//@ stdout: ~K
//@ stdout: calls=5
extern "C" int puts(const char*);
extern "C" int printf(const char*, ...);
inline void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
struct S {
  struct Inner { ~Inner() { puts("~Inner"); } };
  using I = int;
  template <class U> struct TI { ~TI() { puts("~TI"); } };
};
namespace M {
struct K { ~K() { puts("~K"); } };
using J = int;
}
int calls;
template <class T> T* obj(T* p) { ++calls; return p; }
template <class T> void inner(typename T::Inner* p) { obj(p)->T::Inner::~Inner(); }
template <class T> void scalar(typename T::I* p) { obj(p)->T::I::~I(); }
template <class T> void member_template(typename T::template TI<int>* p) {
  obj(p)->T::template TI<int>::~TI();
}
template <class T> void ns_class(T* p) { obj(p)->M::K::~K(); }
template <class T> void ns_scalar(T* p) { obj(p)->M::J::~J(); }
int main() {
  alignas(S::Inner) unsigned char b1[sizeof(S::Inner)];
  inner<S>(::new (b1) S::Inner);
  alignas(S::TI<int>) unsigned char b2[sizeof(S::TI<int>)];
  member_template<S>(::new (b2) S::TI<int>);
  alignas(M::K) unsigned char b3[sizeof(M::K)];
  ns_class(::new (b3) M::K);
  int i = 0;
  scalar<S>(&i);
  ::new (&i) int(1);
  ns_scalar(&i);
  printf("calls=%d\n", calls);
}
