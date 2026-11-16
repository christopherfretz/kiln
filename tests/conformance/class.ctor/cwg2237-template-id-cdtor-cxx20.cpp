// CWG2237 (C++20): a simple-template-id is no longer a constructor or
// destructor declarator-id ([class.ctor.general]/1, [class.dtor]/1). kiln
// warns as GCC 16.1 does (-Wtemplate-id-cdtor; spec/diag.md D18), also for
// the out-of-class `X<T>::~X<T>()`; `X<T>::X()` and `p->~X<int>()` stay
// silent. clang 22 accepts all of it silently. Found running GCC's C++
// testsuite (WP-1466).
//@ expect: run
//@ std: c++20
//@ strict-diags
//@ diag: 18:3 warning "template-id not allowed for constructor in C++20"
//@ diag: 19:3 warning "template-id not allowed for destructor in C++20"
//@ diag: 20:21 warning "template-id not allowed for constructor in C++20"
//@ diag: 24:25 warning "template-id not allowed for destructor in C++20"
//@ diag: 27:34 warning "template-id not allowed for constructor in C++20"
//@ diag: 28:29 warning "template-id not allowed for constructor in C++20"
//@ xfail(clang): clang 22 does not diagnose CWG2237 (spec/diag.md D18)
template<class T> struct X {
  X<T>();
  ~X<T>();
  template<class U> X<T>(U, U);
  struct N { N(); };
};
template<class T> X<T>::X() {}
template<class T> X<T>::~X<T>() {}
template<class T> template<class U> X<T>::X(U, U) {}
template<class T> struct P;
template<class T> struct P<T*> { P<T*>(int) {} };
template<> struct X<char> { X<char>(); };
X<char>::X() {}
template<class T> struct G { G(T) {} };
template<class T> G(T*) -> G<T*>;
void f(X<int> *p) { p->~X<int>(); }
int main() {
  X<int> a;
  X<int> b(1, 2);
  P<int*> c(0);
  X<char> d;
  G g(1);
  G h((int*)0);
}
