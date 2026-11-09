// [expr.prim.req.general]/5, [class.access]/2: only a requirement whose
// access was granted speculatively in the pattern waits for the
// specialization. A non-dependent use outside the requires-expression is
// still checked again in A<long> (a hard error there), and a requirement
// with no friend that may apply, in a template or not, is checked where
// it is written.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 28:43 error "'sn' is a private member of 'B'"
//@ diag: 19:24 note "implicitly declared private here"
//@ diag: 30:24 error "'sn' is a private member of 'B'"
//@ diag: 19:24 note "implicitly declared private here"
//@ diag: 23:50 error "'sn' is a private member of 'B'"
//@ diag: 31:44 note "in instantiation of member function 'A<long>::g' requested here"
//@ diag: 19:24 note "implicitly declared private here"
template <class T> struct A;
class B {
  friend struct A<int>;
  static constexpr int sn = 5;
};
template <class T> struct A {
  static int g() {
    if constexpr (requires { B::sn; }) return B::sn;
    else return 0;
  }
};
template <class T> struct E {
  static constexpr bool v = requires { B::sn; };
};
bool z = requires { B::sn; };
int main() { return A<int>::g() + A<long>::g(); }
