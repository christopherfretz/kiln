// [over.match.funcs.general]/4, [over.match.oper]/2 and clang's enable_if
// attribute: a member function with an explicit object parameter called
// without overload resolution binds the object argument to that parameter;
// one chosen by overload resolution gets the arguments without the object
// argument, bound from the object parameter on (clang CheckEnableIf), and is
// disabled by its first condition when one does not convert or a parameter
// is left without an argument or default argument. A non-member operator
// function binds its first operand to its first parameter.
// Verified against clang 19 and clang 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 52:5 error "no matching member function for call to 'k'"
//@ diag: 33:7 note "candidate disabled: n positive"
//@ diag: 54:7 error "no matching member function for call to 'w'"
//@ diag: 34:7 note "candidate disabled: object positive"
//@ diag: 56:7 error "call to deleted member function 'm'"
//@ diag: 36:7 note "candidate function has been explicitly deleted"
//@ diag: 35:7 note "candidate disabled: shifted"
//@ diag: 57:7 error "call to deleted member function 't'"
//@ diag: 38:26 note "candidate function [with T = S] has been explicitly deleted"
//@ diag: 37:26 note "candidate disabled: never"
//@ diag: 58:5 error "overload resolution selected deleted operator '-'"
//@ diag: 40:7 note "candidate function has been explicitly deleted"
//@ diag: 39:7 note "candidate disabled: n is 2"
//@ diag: 60:6 error "overload resolution selected deleted operator '+'"
//@ diag: 49:5 note "candidate function has been explicitly deleted"
//@ diag: 48:5 note "candidate disabled: a is x1"
struct S;
constexpr int val(const S &);
struct S {
  int v;
  constexpr S(int x) : v(x) {}
  int k(this const S &self, int n) __attribute__((enable_if(n > 0, "n positive")));
  int w(this const S &self, int n) __attribute__((enable_if(val(self) > 0, "object positive")));
  int m(this const S &self, int n = 7) __attribute__((enable_if(n == 7 && val(self) == 5, "shifted")));
  int m(this const S &self, long n) = delete;
  template <class T> int t(this const T &self, int n) __attribute__((enable_if(true, "never")));
  template <class T> int t(this const T &self, long n) = delete;
  int operator-(this S self, int n) __attribute__((enable_if(n == 2, "n is 2")));
  int operator-(this S self, long n) = delete;
};
constexpr int val(const S &s) { return s.v; }
constexpr S pos(3), neg(-1), one(1);
struct X {
  int v;
};
constexpr X x0{0}, x1{1};
int operator+(const X &a, int n) __attribute__((enable_if(a.v == 1, "a is x1")));
int operator+(const X &a, long n) = delete;
void use(S s) {
  s.k(1);
  s.k(0);
  pos.w(0);
  neg.w(0);
  one.m(5);
  one.m(4);
  pos.t(0);
  s - 2;
  x1 + 1;
  x0 + 1;
}
