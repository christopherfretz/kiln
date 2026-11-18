// [class.friend]/9: a friend declaration that names a member of another
// class (a constructor, destructor or conversion function, or a member
// template) must be able to name it: kiln checked only ordinary member
// functions, not these. [class.friend]/4, [temp.friend]/1: a friend
// template naming a constructor or conversion function template that
// does not exist was accepted silently. Same errors as clang 22 (GCC
// 16.1 rejects the same declarations); a friend declared in a class
// template is checked only where clang checks it. Found running GCC's
// C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:13 error "friend function 'A' is a private member of 'A'"
//@ diag: 31:3 note "implicitly declared private here"
//@ diag: 43:13 error "friend function '~A' is a private member of 'A'"
//@ diag: 32:3 note "implicitly declared private here"
//@ diag: 44:13 error "friend function 'operator int' is a private member of 'A'"
//@ diag: 33:3 note "implicitly declared private here"
//@ diag: 45:13 error "friend function 'A' is a private member of 'A'"
//@ diag: 45:13 note "declared private here"
//@ diag: 46:32 error "friend function 'A' is a private member of 'A'"
//@ diag: 46:32 note "declared private here"
//@ diag: 47:32 error "friend function 'operator type-parameter-0-0 *' is a private member of 'A'"
//@ diag: 47:32 note "declared private here"
//@ diag: 48:36 error "friend function 'f' is a private member of 'A'"
//@ diag: 48:36 note "declared private here"
//@ diag: 51:13 error "friend function 'A' is a private member of 'A'"
//@ diag: 31:3 note "implicitly declared private here"
//@ diag: 61:32 error "friend declaration of 'B' does not match any declaration in 'B'"
//@ diag: 62:32 error "friend declaration of 'operator type-parameter-0-0 &' does not match any declaration in 'B'"
class A {
  A(int);
  ~A();
  operator int();
  template <class U> A(U, U);
  template <class U> operator U *();
  template <class U> int f(U);

public:
  template <class U> A(U);
};
class X {
  friend A::A(int);
  friend A::~A();
  friend A::operator int();
  friend A::A(long, long);
  template <class U> friend A::A(U, U);
  template <class U> friend A::operator U *();
  template <class U> friend int A::f(U);
};
template <class T> class XT {
  friend A::A(int);
  template <class U> friend A::A(U, U);
};
XT<int> xt;
struct B {
  template <class U> B(U);
  template <class U> operator U *();
};
class Y {
  template <class U> friend B::B(U);
  template <class U> friend B::B(U, U);
  template <class U> friend B::operator U &();
};
