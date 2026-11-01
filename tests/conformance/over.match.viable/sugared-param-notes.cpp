// [over.match.viable], [over.best.ics]: a candidate constructor's or
// member function's bad-conversion note names the parameter type as
// declared, with its alias sugar and top-level cv ('WI' (aka 'W<int>'),
// 'CI *const'), as for a free function (WP-227).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:3 error "no matching constructor for initialization of 'S'"
//@ diag: 28:3 note "candidate constructor not viable: no known conversion from 'int' to 'WI' (aka 'W<int>') for 1st argument"
//@ diag: 27:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 27:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 2 were provided"
//@ diag: 29:3 note "candidate constructor not viable: requires 3 arguments, but 2 were provided"
//@ diag: 34:3 error "no matching constructor for initialization of 'S'"
//@ diag: 29:3 note "candidate constructor not viable: no known conversion from 'int' to 'I *' (aka 'int *') for 1st argument"
//@ diag: 28:3 note "candidate constructor not viable: requires 2 arguments, but 3 were provided"
//@ diag: 27:8 note "candidate constructor (the implicit copy constructor) not viable: requires 1 argument, but 3 were provided"
//@ diag: 27:8 note "candidate constructor (the implicit move constructor) not viable: requires 1 argument, but 3 were provided"
//@ diag: 36:5 error "no matching member function for call to 'm'"
//@ diag: 30:8 note "candidate function not viable: no known conversion from 'int' to 'WI' (aka 'W<int>') for 1st argument"
//@ diag: 31:8 note "candidate function not viable: requires 2 arguments, but 1 was provided"
//@ diag: 37:5 error "no matching member function for call to 'm'"
//@ diag: 31:8 note "candidate function not viable: no known conversion from 'double' to 'CI *const' (aka 'const int *const') for 1st argument"
//@ diag: 30:8 note "candidate function not viable: requires 1 argument, but 2 were provided"
template <class T> struct W { W(T*); };
using WI = W<int>;
using I = int;
using CI = const int;
struct S {
  S(WI, int);
  S(I*, int, int);
  void m(WI);
  void m(CI* const, int);
};
S s(1, 2);
S t(1, 2, 3);
void g(S& x) {
  x.m(1);
  x.m(1.0, 2);
}
