// [temp.inst]: a member function whose body fails to instantiate is
// invalid; a call naming it (a static member named directly, or one
// reached from another instantiated body) is erroneous without a
// follow-on diagnostic (clang's RecoveryExpr), while a member call and
// an explicit template-id call still fail as constant expressions.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:36 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 28:28 note "in instantiation of member function 'A<int>::f' requested here"
//@ diag: 28:28 error "constexpr variable 'a' must be initialized by a constant expression"
//@ diag: 24:37 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 29:27 note "in instantiation of member function 'A<int>::s' requested here"
//@ diag: 24:37 error "type 'long' cannot be used prior to '::' because it has no members"
//@ diag: 26:36 note "in instantiation of member function 'A<long>::s' requested here"
//@ diag: 30:29 note "in instantiation of member function 'A<long>::u' requested here"
//@ diag: 30:15 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 31:47 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 32:19 note "in instantiation of function template specialization 'g<int>' requested here"
//@ diag: 32:19 error "constexpr variable 'd' must be initialized by a constant expression"
//@ diag: 25:37 error "type 'char' cannot be used prior to '::' because it has no members"
//@ diag: 33:18 note "in instantiation of member function 'A<char>::t' requested here"
template <class T> struct A {
  constexpr int f() const { return T::value; }
  static constexpr int s() { return T::value + 1; }
  static constexpr int t() { return T::value + 2; }
  constexpr int u() const { return s(); }
};
constexpr int a = A<int>().f();
constexpr int b = A<int>::s();
constexpr int c = A<long>().u();
template <class T> constexpr int g() { return T::v; }
constexpr int d = g<int>();
int e = A<char>::t();
