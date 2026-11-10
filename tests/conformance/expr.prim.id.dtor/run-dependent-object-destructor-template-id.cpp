// [basic.lookup.qual.general]/2, /4.4, [expr.prim.id.dtor]/2: in
// `p->~TI<int>()` on a dependent object the template-name `TI` is a
// member-qualified name; when it names nothing at the definition it is
// looked up in the object's class at instantiation, where the
// injected-class-name of `N::TI<int>` finds the class template
// ([temp.local]/1). Both `->` and `.` forms (WP-1173).
//@ expect: run
//@ strict-diags
//@ stdout: ~TI
//@ stdout: ~TI
//@ stdout: done
extern "C" int puts(const char*);
inline void* operator new(decltype(sizeof 0), void* p) noexcept { return p; }
namespace N {
template <class X> struct TI { ~TI() { puts("~TI"); } };
}
template <class T> void arrow(T* p) { p->~TI<int>(); }
template <class T> void dot(T& r) { r.~TI<int>(); }
int main() {
  alignas(N::TI<int>) unsigned char buf[sizeof(N::TI<int>)];
  arrow(new (buf) N::TI<int>);
  dot(*new (buf) N::TI<int>);
  puts("done");
}
