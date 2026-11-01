// [temp.deduct.call]/4: after deduction the substituted parameter type ('deduced A') must equal the
// adjusted argument type up to the allowed differences (extra cv, qualification conversion, derived
// class of a simple-template-id); otherwise the candidate is ignored with clang's 'deduced type' note
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:3 error "no matching function for call to 'g'"
//@ diag: 10:24 note "of 2nd parameter does not match adjusted type 'pr<int, long> *' of argument [with T = int]"
template<class A, class B> struct pr {};
template<class T> struct id { using type = T; };
template<class T> void g(int, pr<T, typename id<T>::type> *);
template<class T> void p(T*, typename id<T>::type *);
template<class T> struct B {};
struct D : B<int> {};
template<class T> void k(B<T>);
template<class T> void m(const T*);
template<class T> void n(T&&);
template<class T> void q(T, void (*)(typename id<T>::type) noexcept);
void fn(int);
void t() {
  g(1, (pr<int, long>*)0);
  long l = 0; p(&l, &l);
  k(D{});
  int* ip = 0; m(ip);
  int i = 0; n(i);
}
