// [temp.names]/3, [temp.arg.explicit]: in C++20 an ADL-only call with
// explicit template arguments is silent by default (-Wpre-c++20-compat is
// off); only the call's own errors remain.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 19:9 error "no matching function for call to 'q'"
//@ diag: 14:25 note "candidate function template not viable: no known conversion from 'M::S' to 'int' for 2nd argument"
//@ diag: 13:25 note "candidate function template not viable: requires 1 argument, but 2 were provided"
//@ diag: 20:9 error "use of undeclared identifier 'r'"
namespace M {
  struct S {};
  template<class T> int q(S);
  template<class T> int q(S, int);
  int r(S);
}
int a = q<int>(M::S{});
int b = q<long>(M::S{}, 1);
int c = q<int>(M::S{}, M::S{});
int d = r<int>(M::S{});
template<class U> int h(U u) { return q<int>(u); }
int e = h(M::S{});
template<class U> int h2() { return q<int>(M::S{}); }
int f = h2<int>();
