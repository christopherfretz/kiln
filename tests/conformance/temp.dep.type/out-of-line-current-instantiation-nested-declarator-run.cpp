// [temp.dep.type]/6, [dcl.meaning]/1, [basic.lookup.unqual]: before the
// declarator-id of an out-of-line member of a class template, a
// `typename C<X>::T` names the member of the current instantiation also
// when it is nested in a function type, an array, a member pointer or
// declarator parentheses, inside a template argument
// (`W<void(typename C<X>::T)>`); the suffixes after a parenthesized
// qualified declarator-id (`X (*C<X>::f())(T)`) are in the class's scope.
//@ expect: run
//@ strict-diags
//@ stdout: 7 7 4 2 5 7 8 1 3
extern "C" int printf(const char*, ...);
template <class X> struct W { using type = X; };
template <class X> struct C {
  using T = X;
  T v = 7;
  static T arr[2];
  static T (*sfp)(T);
  T* p();
  T (*fp())(T);
  T (*fq())(T);
  W<void(T)> wf();
  W<T[2]> wa();
  W<T C::*> wm();
  W<T (*)(T)> wp();
  T C::* mp();
  T (C::*mfp())(T);
  T (&ar())[2];
  T get(T x) { return x + v; }
  static T id(T x) { return x; }
};
template <class X> typename C<X>::T C<X>::arr[2] = {1, 2};
template <class X> typename C<X>::T (*C<X>::sfp)(typename C<X>::T) = &C<X>::id;
template <class X> typename C<X>::T* C<X>::p() { return &v; }
template <class X> typename C<X>::T (*C<X>::fp())(typename C<X>::T) { return &C<X>::id; }
template <class X> X (*C<X>::fq())(T) { return &id; }
template <class X> W<void(typename C<X>::T)> C<X>::wf() { return {}; }
template <class X> W<typename C<X>::T[2]> C<X>::wa() { return {}; }
template <class X> W<typename C<X>::T C<X>::*> C<X>::wm() { return {}; }
template <class X> W<typename C<X>::T (*)(typename C<X>::T)> C<X>::wp() { return {}; }
template <class X> typename C<X>::T C<X>::* C<X>::mp() { return &C<X>::v; }
template <class X> typename C<X>::T (C<X>::*C<X>::mfp())(typename C<X>::T) { return &C<X>::get; }
template <class X> typename C<X>::T (&C<X>::ar())[2] { return arr; }
int main() {
  C<int> c;
  c.wf(); c.wa(); c.wm(); c.wp();
  printf("%d %d %d %d %d %d %d %d %d\n", *c.p(), c.fp()(7), c.fq()(4), C<int>::arr[1],
         C<int>::sfp(5), c.*c.mp(), (c.*c.mfp())(1), c.ar()[0], C<int>::arr[0] + c.ar()[1]);
}
