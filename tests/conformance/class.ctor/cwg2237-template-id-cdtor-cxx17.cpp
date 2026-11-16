// CWG2237 ([class.ctor.general]/1, [class.dtor]/1 before C++20): a
// simple-template-id naming the class is a constructor or destructor
// declarator-id (`X<T>();`, `~X<T>();`), also for a constructor template, a
// partial specialization's own name and an explicit specialization; the
// out-of-class forms and `p->~X<int>()` are not affected, and a deduction
// guide `G(T*) -> G<T*>` is no constructor. Silent before C++20 (GCC 16.1,
// clang 22). Found running GCC's C++ testsuite (WP-1466).
//@ expect: run
//@ std: c++17
//@ strict-diags
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
