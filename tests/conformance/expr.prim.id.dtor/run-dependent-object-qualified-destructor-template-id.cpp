// [basic.lookup.qual.general]/2, /4.2, [expr.prim.id.dtor]/2: in
// `p->Self::template TI<int>::~TI<int>()` on a dependent object, `Self` is
// a member-qualified name found in the object's class at instantiation
// (`S::TI<int>::Self` is `S`), and the destructor's template-name `TI` is
// looked up in the prefix `S` (4.2), where it names `S::TI`, the
// qualifier's class template. The form is kept dependent at the definition
// and checked at instantiation (WP-1192), also inside a member template of
// a class template, inside a generic lambda (whose object stays dependent
// until the call operator is instantiated), and with an object expression
// that is evaluated exactly once. clang 22 accepts; clang 19 does not look
// `Self` up in the object's class, hence clang>=22.
//@ expect: run
//@ strict-diags
//@ requires: clang>=22
//@ stdout: ~TI
//@ stdout: ~TI
//@ stdout: ~TI
//@ stdout: ~TI
//@ stdout: calls=1
extern "C" int printf(const char*, ...);
inline void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
struct S {
  template <class X> struct TI { using Self = S; ~TI() { printf("~TI\n"); } };
};
template <class T> void a(T* p) { p->Self::template TI<int>::~TI<int>(); }
template <class T> struct Outer {
  template <class U> void m(U* p) { p->Self::template TI<T>::~TI<T>(); }
};
template <class T> void lam(T* p) {
  [](auto* q) { q->Self::template TI<int>::~TI<int>(); }(p);
}
int calls;
template <class T> T* obj(T* p) { ++calls; return p; }
template <class T> void ev(T* p) { obj(p)->Self::template TI<int>::~TI<int>(); }
int main() {
  alignas(S::TI<int>) unsigned char buf[sizeof(S::TI<int>)];
  a(new (buf) S::TI<int>);
  Outer<int>().m(new (buf) S::TI<int>);
  lam(new (buf) S::TI<int>);
  ev(new (buf) S::TI<int>);
  printf("calls=%d\n", calls);
}
