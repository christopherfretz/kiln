// [basic.lookup.qual.general]/2, /4.2, [expr.prim.id.dtor]/2: in
// `p->Self::template TI<int>::~TI<int>()` on a dependent object, `Self` is
// a member-qualified name found in the object's class at instantiation
// (`S::TI<int>::Self` is `S`), and the destructor's template-name `TI` is
// looked up in the prefix `S` (4.2), where it names `S::TI`, the
// qualifier's class template. clang 22 accepts; clang 19 does not look
// `Self` up in the object's class, hence clang>=22. kiln rejects at the
// definition ("destructor name 'TI' does not refer to a template"): the
// template-name after a dependent prefix that names nothing at the
// definition is not kept for instantiation (WP-1173).
//@ expect: run
//@ strict-diags
//@ requires: clang>=22
//@ xfail(kiln): WP-1173 follow-up, `~Q<args>` after a dependent unresolved prefix is not deferred to instantiation
//@ stdout: ~TI
extern "C" int puts(const char*);
inline void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
struct S {
  template <class X> struct TI { using Self = S; ~TI() { puts("~TI"); } };
};
template <class T> void a(T* p) { p->Self::template TI<int>::~TI<int>(); }
int main() {
  alignas(S::TI<int>) unsigned char buf[sizeof(S::TI<int>)];
  a(new (buf) S::TI<int>);
}
