// [basic.lookup.qual.general]/2, /4.2, /4.5, [expr.prim.id.dtor]/2,
// [expr.ref]/4: `p->Self::template TI<int>::~Q<args>()` on a dependent
// object is kept dependent at the definition and checked at instantiation
// (WP-1192), where `Self` is found in the object's class and `Q` is looked
// up in the prefix `S` (4.2). Each mismatch gets clang's diagnostic:
// `~TI<long>` and `~TJ<int>` name no destructor of `S::TI<int>`; `V` and
// `W` are not templates; `Nope` is not a member of `S`; the first
// type-name `S::TJ<int>` is not the object's class; `~Inner` after the
// template-id prefix is not a member of `S::TI<int>`. A non-template first
// type-name that is not a class (`Self::I::~I` with `I` = int) is diagnosed
// as such, and a first type-name not found at the definition (`Inner`) is
// diagnosed there even though the object is dependent. clang 19 does not
// look `Self` up in the object's class, hence clang>=22.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 54:38 error "'Inner' does not refer to a type name in pseudo-destructor expression; expected the name of type 'T'"
//@ diag: 46:38 error "no member named '~TI' in 'S::TI<int>'"
//@ diag: 56:10 note "in instantiation of function template specialization 'a<S::TI<int>>' requested here"
//@ diag: 47:63 error "'V' does not refer to a template"
//@ diag: 56:16 note "in instantiation of function template specialization 'b<S::TI<int>>' requested here"
//@ diag: 39:9 note "declared as a non-template here"
//@ diag: 48:63 error "no member named 'Nope' in 'S'"
//@ diag: 56:22 note "in instantiation of function template specialization 'c<S::TI<int>>' requested here"
//@ diag: 49:38 error "no member named '~TJ' in 'S::TI<int>'"
//@ diag: 56:28 note "in instantiation of function template specialization 'd<S::TI<int>>' requested here"
//@ diag: 50:38 error "'S::TJ<int>::~TJ' is not a member of class 'S::TI<int>'"
//@ diag: 56:34 note "in instantiation of function template specialization 'e<S::TI<int>>' requested here"
//@ diag: 51:63 error "'W' does not refer to a template"
//@ diag: 56:40 note "in instantiation of function template specialization 'g<S::TI<int>>' requested here"
//@ diag: 40:10 note "declared as a non-template here"
//@ diag: 52:63 error "no member named '~Inner' in 'S::TI<int>'"
//@ diag: 56:46 note "in instantiation of function template specialization 'h<S::TI<int>>' requested here"
//@ diag: 53:44 error "'Self::I' (aka 'int') is not a class, namespace, or enumeration"
//@ diag: 57:3 note "in instantiation of function template specialization 'k<S::Inner>' requested here"
struct S {
  template <class X> struct TI { using Self = S; ~TI(); };
  template <class X> struct TJ { ~TJ(); };
  using V = int;
  struct W { ~W(); };
  struct Inner { using Self = S; ~Inner(); };
  using I = int;
};
// Valid: `TI<int>` is the object's class, `~TI<int>` its destructor.
template <class T> void ok(T* p) { p->Self::template TI<int>::~TI<int>(); }
template <class T> void a(T* p) { p->Self::template TI<int>::~TI<long>(); }
template <class T> void b(T* p) { p->Self::template TI<int>::~V<int>(); }
template <class T> void c(T* p) { p->Self::template TI<int>::~Nope<int>(); }
template <class T> void d(T* p) { p->Self::template TI<int>::~TJ<int>(); }
template <class T> void e(T* p) { p->Self::template TJ<int>::~TJ<int>(); }
template <class T> void g(T* p) { p->Self::template TI<int>::~W<int>(); }
template <class T> void h(T* p) { p->Self::template TI<int>::~Inner(); }
template <class T> void k(T* p) { p->Self::I::~I(); }
template <class T> void m(T* p) { p->Inner::~Inner(); }
void f(S::TI<int>* p, S::Inner* q) {
  ok(p); a(p); b(p); c(p); d(p); e(p); g(p); h(p);
  k(q);
}
