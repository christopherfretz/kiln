// [dcl.attr.deprecated]/4 (WP-297): naming a deprecated class, class
// template specialization, enumeration or enumerator warns, also through a
// typedef or alias (clang ShouldDiagnoseAvailabilityOfDecl looks through to
// the tag; the note names it), and an explicit specialization has its own
// attributes.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 36:9 warning "'S' is deprecated"
//@ diag: 30:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 37:12 warning "'P<int>' is deprecated: gone"
//@ diag: 31:28 note "'P<int>' has been explicitly marked deprecated here"
//@ diag: 40:3 warning "'S' is deprecated"
//@ diag: 30:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 41:3 warning "'P<int>' is deprecated: gone"
//@ diag: 31:28 note "'P<int>' has been explicitly marked deprecated here"
//@ diag: 42:3 warning "'Q<char>' is deprecated"
//@ diag: 33:21 note "'Q<char>' has been explicitly marked deprecated here"
//@ diag: 44:3 warning "'E' is deprecated"
//@ diag: 34:8 note "'E' has been explicitly marked deprecated here"
//@ diag: 44:9 warning "'e0' is deprecated"
//@ diag: 34:8 note "'E' has been explicitly marked deprecated here"
//@ diag: 45:11 warning "'f0' is deprecated"
//@ diag: 35:15 note "'f0' has been explicitly marked deprecated here"
//@ diag: 46:3 warning "'TS' is deprecated"
//@ diag: 30:10 note "'S' has been explicitly marked deprecated here"
//@ diag: 47:3 warning "'AP' is deprecated: gone"
//@ diag: 31:28 note "'P<int>' has been explicitly marked deprecated here"
//@ diag: 48:5 warning "'Q<char>' is deprecated"
//@ diag: 33:21 note "'Q<char>' has been explicitly marked deprecated here"
struct [[deprecated]] S { S(int) {} void f() {} };
template<class T> struct [[deprecated("gone")]] P { P() {} void g() {} };
template<class T> struct Q { Q() {} };
template<> struct [[deprecated]] Q<char> { Q() {} void h() {} };
enum [[deprecated]] E { e0 };
enum F { f0 [[deprecated]], f1 };
typedef S TS;
using AP = P<int>;
template<class T> struct W : T { using T::T; };
void use() {
  S s(1);
  P<int> p;
  Q<char> q;
  Q<int> qi;
  E e = e0;
  int i = f0;
  TS ts(2);
  AP ap;
  W<Q<char>> w;
  s.f(); p.g(); q.h();
  (void)ts; (void)ap; (void)e; (void)i; (void)qi; (void)w;
}
