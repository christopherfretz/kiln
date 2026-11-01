// [temp.friend]: a befriended function template specialization f<int> has
// access, f<char> does not; a befriended template's every specialization does.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:44 error "'x' is a private member of 'A'"
//@ diag: 12:7 note "implicitly declared private here"
//@ diag: 18:44 error "'x' is a private member of 'A'"
//@ diag: 21:20 note "in instantiation of function template specialization 'f<char>' requested here"
//@ diag: 12:7 note "implicitly declared private here"
template<class T> void f(T);
class A {
  int x;
  friend void f<int>(int);
  template<class U> friend void g(U);
public:
  A();
};
template<class T> void f(T) { A a; (void)a.x; }
template<class T> void g(T) { A a; (void)a.x; }
template<class T> void h(T) { A a; (void)a.x; }
void use() { f(1); f('c'); g(1); g('c'); h(1); }
