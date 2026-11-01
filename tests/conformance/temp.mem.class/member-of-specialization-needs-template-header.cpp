// [temp.expl.spec]/1: a member of an implicitly instantiated class template specialization is
// explicitly specialized only after `template<>`; the members of an explicit specialization
// are defined without it ([temp.expl.spec]/5)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:14 error "template specialization requires 'template<>'"
//@ diag: 17:13 error "template specialization requires 'template<>'"
//@ diag: 18:16 error "template specialization requires 'template<>'"
//@ diag: 19:22 error "template specialization requires 'template<>'"
template <class T> struct A {
  void f();
  static int s;
  struct F;
  enum class E;
};
void A<int>::f() {}
int A<int>::s = 1;
struct A<int>::F {};
enum class A<short>::E { x };
template <> struct A<char> {
  void f();
  struct F;
};
void A<char>::f() {}
struct A<char>::F {};
template <> struct A<long>::F {};
