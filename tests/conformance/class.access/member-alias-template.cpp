// [class.access]/4, [temp.alias]: a member alias template is access-checked like any
// member, and clang locates it at its 'using' keyword for "declared private here"; a
// private member class template named as a nested-name-specifier component
// (`P::C<int>::B`) is checked before its '::' like any other component.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 31:4 error "'A' is a private member of 'P'"
//@ diag: 23:22 note "implicitly declared private here"
//@ diag: 32:4 error "'B' is a private member of 'P'"
//@ diag: 24:9 note "implicitly declared private here"
//@ diag: 33:4 error "'C' is a private member of 'P'"
//@ diag: 25:29 note "implicitly declared private here"
//@ diag: 34:20 error "'A' is a private member of 'P'"
//@ diag: 23:22 note "implicitly declared private here"
//@ diag: 36:52 error "'C' is a private member of 'P'"
//@ diag: 37:6 note "in instantiation of template class 'S<P>' requested here"
//@ diag: 25:29 note "implicitly declared private here"
//@ diag: 38:34 error "'C' is a private member of 'P'"
//@ diag: 25:29 note "implicitly declared private here"
//@ diag: 39:4 error "'D' is a protected member of 'P'"
//@ diag: 28:3 note "declared protected here"
class P {
  template <class T> using A = T;
  using B = int;
  template <class T> struct C { using B = T; };
protected:
  template <class T>
  using D = T *;
  friend struct Fr;
};
P::A<int> x1;
P::B x2;
P::C<int>::B x3;
template <class T> using X = typename T::template A<int>;
X<P> x5;
template <class T> struct S { typename T::template C<T>::B m; };
S<P> x6;
struct Q : P { D<int> ok; static C<int>::B ok2; };
P::D<int> x7;
struct Fr { P::A<int> ok; P::C<long>::B ok2; };
