// clang's `enable_if` attribute (clang AttrDocs "enable_if"): a candidate
// whose condition is false or not a constant expression with the call's
// arguments is not viable, noted as "candidate disabled: <message>"; a call
// without overload resolution (a member call, one function) is checked too;
// the attribute applies only to functions.
// Verified against clang 19 and clang 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 37:22 warning "'enable_if' attribute only applies to functions"
//@ diag: 40:3 error "no matching function for call to 'f'"
//@ diag: 26:6 note "candidate disabled: only literal zero"
//@ diag: 41:3 error "no matching function for call to 't'"
//@ diag: 28:3 note "candidate disabled: <no message provided>"
//@ diag: 42:3 error "no matching conversion for functional-style cast from 'int' to 'S'"
//@ diag: 29:8 note "candidate constructor (the implicit copy constructor) not viable: no known conversion from 'int' to 'const S' for 1st argument"
//@ diag: 29:8 note "candidate constructor (the implicit move constructor) not viable: no known conversion from 'int' to 'S' for 1st argument"
//@ diag: 33:3 note "candidate disabled: zero"
//@ diag: 30:3 note "candidate constructor not viable: requires 0 arguments, but 1 was provided"
//@ diag: 43:7 error "no matching member function for call to 'm'"
//@ diag: 31:7 note "candidate disabled: one"
//@ diag: 44:3 error "call to 'a' is ambiguous"
//@ diag: 35:6 note "candidate function"
//@ diag: 36:6 note "candidate function"
//@ diag: 45:3 error "no matching function for call to 'f'"
//@ diag: 26:6 note "candidate disabled: only literal zero"
void f(int n) __attribute__((enable_if(n == 0, "only literal zero")));
template <class T>
T t(T x) __attribute__((enable_if(x == 0, "")));
struct S {
  S() = default;
  int m(int n) const __attribute__((enable_if(n == 1, "one")));
  template <class U>
  S(U u) __attribute__((enable_if(u == 0, "zero")));
};
void a(int n) __attribute__((enable_if(n > 0, "positive")));
void a(int n) __attribute__((enable_if(n < 10, "small")));
int x __attribute__((enable_if(1, "x")));
void g(int v) {
  f(0);
  f(1);
  t(1);
  S(1);
  S().m(2);
  a(5);
  f(v);
}
