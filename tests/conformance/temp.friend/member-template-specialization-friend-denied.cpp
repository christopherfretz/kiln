// [temp.friend]/1.2: `friend void A::f(int);` naming a member function
// template befriends only the specialization the type deduces, `A::f<int>`
// (found running GCC's C++ testsuite): `A::f<long>` is not a friend. Where
// `A` also declares a non-template `g(int)`, that is the friend, not the
// template's specialization `A::g<int>`. A private member template's
// specialization is not accessible to befriend ([class.friend]/9).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:18 error "friend function 'p' is a private member of 'A'"
//@ diag: 28:18 note "declared private here"
//@ diag: 29:18 error "friend function 'p' is a private member of 'A'"
//@ diag: 29:18 note "declared private here"
//@ diag: 32:40 error "'x' is a private member of 'B'"
//@ diag: 30:14 note "implicitly declared private here"
//@ diag: 33:39 error "'x' is a private member of 'B'"
//@ diag: 30:14 note "implicitly declared private here"

struct A {
  template <class T> void f(T);
  void g(int);
  template <class T> void g(T);
private:
  template <class T> void p(T);
};
class B {
  friend void A::f(int);
  friend void A::g(int);
  friend void A::p(int);
  friend void A::p<>(char);
  static int x;
};
template <> void A::f(long) { (void)B::x; }
template <> void A::g(int) { (void)B::x; }
template <> void A::f(int) { (void)B::x; }
void A::g(int) { (void)B::x; }
