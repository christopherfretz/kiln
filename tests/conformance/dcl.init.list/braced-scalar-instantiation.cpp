// [dcl.init.list]/3.10: a scalar is list-initialized from the single
// element of its braced-init-list. -Wbraced-scalar-init flags the braces
// where the code is written; an instantiation does not repeat it for a
// dependent list (clang is silent there), but too many braces still are.
//@ args: -Wbraced-scalar-init -Wmany-braces-around-scalar-init
//@ strict-diags
//@ diag: 11:53 warning "braces around scalar initializer"
//@ diag: 15:44 warning "too many braces around scalar initializer"
//@ diag: 24:3 note "in instantiation of function template specialization 'f6<int>' requested here"
template <class T> bool f1(T* a, T* b) { return {a == b}; }
template <class T> bool f2(int* a, int* b) { return {a == b}; }
template <class T> int f3(T a) { return {a}; }
template <class T> void g(T);
template <class T> void f4(T a) { g<int>({a}); }
template <class T> void f6(T a) { int x = {{a}}; (void)x; }
template <class T> struct P {
  friend constexpr bool operator==(P, T* q) { return {q == nullptr}; }
};
void h(int* p) {
  f1(p, p);
  f2<int>(p, p);
  f3(1);
  f4(1);
  f6(1);
  (void)(P<int>() == p);
}
