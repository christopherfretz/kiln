// [temp.inst], [over.match.call], [dcl.init]: a variable whose initializer
// is a call that fails overload resolution (or a member function call whose
// arguments do not convert) stays valid at instantiation, as clang's
// RecoveryExpr keeps it: later statements that use it are still checked and
// diagnosed (WP-849; kiln marked it failed and missed them). Only a
// placeholder type is invalidated by such an initializer. The recovery
// takes the candidates' common return type: a `void` one does not
// initialize the variable. A call of a single qualified function is no
// recovery: its variable is invalid and later uses are silent.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 74:11 error "no matching function for call to 'f'"
//@ diag: 111:12 note "in instantiation of function template specialization 'conditional<X>' requested here"
//@ diag: 60:5 note "candidate function not viable: no known conversion from 'X' to 'int' for 1st argument"
//@ diag: 61:5 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 75:17 error "incompatible operand types ('X' and 'int')"
//@ diag: 78:12 error "no matching function for call to 'f'"
//@ diag: 112:12 note "in instantiation of function template specialization 'deduced<X>' requested here"
//@ diag: 60:5 note "candidate function not viable: no known conversion from 'X' to 'int' for 1st argument"
//@ diag: 61:5 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 80:17 error "incompatible operand types ('X' and 'int')"
//@ diag: 83:11 error "no matching function for call to 'q'"
//@ diag: 113:12 note "in instantiation of function template specialization 'differing<X>' requested here"
//@ diag: 62:6 note "candidate function not viable: no known conversion from 'X' to 'int' for 1st argument"
//@ diag: 63:5 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 84:5 error "no viable conversion from 'int' to 'X'"
//@ diag: 59:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const X &' for 1st argument"
//@ diag: 59:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'X &&' for 1st argument"
//@ diag: 88:13 error "no matching member function for call to 'm'"
//@ diag: 114:12 note "in instantiation of function template specialization 'member_set<X>' requested here"
//@ diag: 68:7 note "candidate function not viable: no known conversion from 'X' to 'int' for 1st argument"
//@ diag: 69:7 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 89:5 error "no viable conversion from 'int' to 'X'"
//@ diag: 59:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const X &' for 1st argument"
//@ diag: 59:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'X &&' for 1st argument"
//@ diag: 93:20 error "no viable conversion from 'X' to 'int'"
//@ diag: 115:12 note "in instantiation of function template specialization 'member_single<X>' requested here"
//@ diag: 70:17 note "passing argument to parameter here"
//@ diag: 94:5 error "no viable conversion from 'int' to 'X'"
//@ diag: 59:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const X &' for 1st argument"
//@ diag: 59:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'X &&' for 1st argument"
//@ diag: 98:11 error "no matching function for call to 'v'"
//@ diag: 116:12 note "in instantiation of function template specialization 'void_result<X>' requested here"
//@ diag: 64:6 note "candidate function not viable: no known conversion from 'X' to 'int' for 1st argument"
//@ diag: 65:6 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 98:7 error "cannot initialize a variable of type 'int' with an rvalue of type 'void'"
//@ diag: 99:5 error "no viable conversion from 'int' to 'X'"
//@ diag: 59:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const X &' for 1st argument"
//@ diag: 59:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'X &&' for 1st argument"
//@ diag: 103:11 error "no matching function for call to 'f'"
//@ diag: 117:12 note "in instantiation of function template specialization 'nested<X>' requested here"
//@ diag: 60:5 note "candidate function not viable: no known conversion from 'X' to 'int' for 1st argument"
//@ diag: 61:5 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 104:12 error "invalid operands to binary expression ('int' and 'X')"
//@ diag: 107:17 error "no viable conversion from 'X' to 'int'"
//@ diag: 118:12 note "in instantiation of function template specialization 'qualified<X>' requested here"
//@ diag: 66:12 note "passing argument to parameter here"
struct X {};
int f(int);
int f(double, int);
long q(int);
int q(double, int);
void v(int);
void v(double, int);
int one(int);
struct S {
  int m(int);
  int m(double, int);
  int single(int);
};

template <class T> T conditional(int k) {
  int n = f(T());
  return n == 1 ? T() : 0;
}
template <class T> T deduced(int k) {
  auto n = f(T());
  T z = n;
  return k == 1 ? T() : 0;
}
template <class T> T differing(int) {
  int n = q(T());
  T z = n;
  return z;
}
template <class T> T member_set(S s) {
  int n = s.m(T());
  T z = n;
  return z;
}
template <class T> T member_single(S s) {
  int n = s.single(T());
  T z = n;
  return z;
}
template <class T> T void_result(int) {
  int n = v(T());
  T z = n;
  return z;
}
template <class T> T nested(int) {
  int n = f(T()) + 1;
  return n + T();
}
template <class T> T qualified(int) {
  int n = ::one(T());
  T z = n;
  return z;
}
template X conditional<X>(int);
template X deduced<X>(int);
template X differing<X>(int);
template X member_set<X>(S);
template X member_single<X>(S);
template X void_result<X>(int);
template X nested<X>(int);
template X qualified<X>(int);
