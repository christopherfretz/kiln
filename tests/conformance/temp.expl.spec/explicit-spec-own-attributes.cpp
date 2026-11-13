// [temp.expl.spec], [dcl.attr.deprecated]/4, [dcl.attr.nodiscard]/2: an
// explicit specialization of a function template (or of a member of a class
// template, or of a member template) takes the attributes written on it;
// those of the primary template still apply to it as well, as clang 19 and
// 22 merge the specialization with the declaration instantiated from the
// template (GCC 16.1 keeps the primary's `[[deprecated]]` but not its
// `[[nodiscard]]`; kiln follows clang).
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wall
//@ diag: 63:3 warning "'f<int>' is deprecated"
//@ diag: 34:15 note "'f<int>' has been explicitly marked deprecated here"
//@ diag: 63:13 warning "'f<long>' is deprecated: msg"
//@ diag: 35:15 note "'f<long>' has been explicitly marked deprecated here"
//@ diag: 64:3 warning "'g<int>' is deprecated"
//@ diag: 36:22 note "'g<int>' has been explicitly marked deprecated here"
//@ diag: 64:13 warning "'g<char>' is deprecated"
//@ diag: 36:22 note "'g<char>' has been explicitly marked deprecated here"
//@ diag: 65:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 65:13 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 66:3 warning "ignoring return value of function declared with 'nodiscard' attribute"
//@ diag: 66:13 warning "ignoring return value of function declared with 'nodiscard' attribute: why"
//@ diag: 66:24 warning "ignoring return value of function declared with 'warn_unused_result' attribute"
//@ diag: 67:3 warning "'gd<int>' is deprecated"
//@ diag: 45:28 note "'gd<int>' has been explicitly marked deprecated here"
//@ diag: 68:7 warning "'m<int>' is deprecated"
//@ diag: 49:15 note "'m<int>' has been explicitly marked deprecated here"
//@ diag: 69:12 warning "'m' is deprecated"
//@ diag: 54:15 note "'m' has been explicitly marked deprecated here"
//@ diag: 70:12 warning "'t<int>' is deprecated"
//@ diag: 55:27 note "'t<int>' has been explicitly marked deprecated here"
template <class T> void f();
template <> [[deprecated]] void f<int>();
template <> [[deprecated("msg")]] void f<long>();
template <class T> [[deprecated]] void g();
template <> void g<int>();
template <class T> [[nodiscard]] int h();
template <> int h<int>();
template <class T> int k();
template <> [[nodiscard]] int k<int>();
template <> [[nodiscard("why")]] int k<long>();
template <> __attribute__((warn_unused_result)) int k<char>();
template <class T> void gd();
template <> __attribute__((deprecated)) void gd<int>();
struct S {
  template <class T> void m();
};
template <> [[deprecated]] void S::m<int>();
template <class U> struct C {
  void m();
  template <class T> void t();
};
template <> [[deprecated]] void C<int>::m();
template <> template <> [[deprecated]] void C<int>::t<int>();
template <class T> void nr3();
template <> __attribute__((noreturn)) void nr3<int>();
int x3() { nr3<int>(); }
template <class T> [[noreturn]] void nr4();
template <> void nr4<int>();
int x4() { nr4<int>(); }
int use() {
  f<int>(); f<long>(); f<char>();
  g<int>(); g<char>();
  h<int>(); h<char>();
  k<int>(); k<long>(); k<char>(); k<short>();
  gd<int>();
  S{}.m<int>();
  C<int>{}.m(); C<long>{}.m();
  C<int>{}.t<int>();
  return 0;
}
