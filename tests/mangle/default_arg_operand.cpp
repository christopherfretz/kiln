// Mangling differential: implicit parts of a non-dependent subexpression
// of a dependent operand ([temp.over.link]/5, [dcl.fct.default],
// [expr.new]/24; spec/abi.md "Mangling", WP-1214). clang 19 and 22:
// - a default argument mangles as its expression (`h()` is
//   `clL_Z1hiELi3EE` for `int h(int = 3)`); a one-argument functional
//   cast keeps only its first argument (`B(3)` is `cv1BLi3E`), a
//   temporary object all of them (`B()` is `cv1B_Li1ELi2EE`);
// - a new-expression's initializer is `pi`/`il` with the constructor's
//   arguments inlined, default arguments too, and a default-initialized
//   class has an implicit constructor call (`new Q` is `nw_1QpiE`, `new
//   int` `nw_iE`);
// - a braced list mangles as written, without the value- and
//   default-member-initialized members (`A{}` is `tl1AE`).
// `new B(3)` and `new B{3}` are distinct expressions ([temp.over.link]/5).
int h(int = 3);
struct S {
  S(int, int = 2);
};
struct A {
  int x = 5;
  int y;
};
struct P {
  int a;
  int b;
};
struct B {
  B(int = 1, int = 2);
};
struct Q {
  int q;
};
template <class T> struct X {
  int v;
};
template <class T> int k(T, int = 4);
template <class T> auto f1(T t) -> decltype(t + h()) { return 0; }
template <class T> auto f2(T t) -> decltype(t, S(1), 0) { return 0; }
template <class T> auto f3(T t) -> decltype(k(t)) { return 0; }
template <class T> auto f4(T t) -> decltype(t, k(1)) { return 0; }
template <class T> auto g1(T t) -> decltype(t, A{1}) { return {}; }
template <class T> auto g2(T t) -> decltype(t, B()) { return {}; }
template <class T> auto g3(T t) -> decltype(t, B{}) { return {}; }
template <class T> auto g5(T t) -> decltype(t, B(3)) { return {}; }
template <class T> auto g6(T t) -> decltype(t, A{}) { return {}; }
template <class T> auto g7(T t) -> decltype(t, P{1}) { return {}; }
template <class T> auto n1(T t) -> decltype(t, new B(3)) { return {}; }
template <class T> auto n2(T t) -> decltype(t, new B{3}) { return {}; }
template <class T> auto n3(T t) -> decltype(t, new B{}) { return {}; }
template <class T> auto n4(T t) -> decltype(t, new B()) { return {}; }
template <class T> auto n5(T t) -> decltype(t, new int(4)) { return {}; }
template <class T> auto n6(T t) -> decltype(t, new B(t)) { return {}; }
template <class T> auto n7(T t) -> decltype(t, new B) { return {}; }
template <class T> auto n8(T t) -> decltype(t, new A) { return {}; }
template <class T> auto n9(T t) -> decltype(t, new int) { return {}; }
template <class T> auto m1(T t) -> decltype(t, new Q) { return {}; }
template <class T> auto m2(T t) -> decltype(t, new P{1}) { return {}; }
template <class T> auto m3(T t) -> decltype(t, new B[2]) { return {}; }
template <class T> auto m4(T t) -> decltype(t, new Q[2]) { return {}; }
template <class T> auto m5(T t) -> decltype(t, new T) { return {}; }
template <class T> auto m6(T t) -> decltype(t, new X<T>) { return {}; }
template <class T> auto m7(T t) -> decltype(t, new X<int>) { return {}; }
template <class T> auto m8(T t) -> decltype(t, new int()) { return {}; }
template <class T> auto m9(T t) -> decltype(t, new Q()) { return {}; }
template <class T> auto ma(T t) -> decltype(t, new Q{}) { return {}; }
template <class T> auto mb(T t) -> decltype(t, new B[2]{}) { return {}; }
void use() {
  f1(1); f2(1); f3(1); f4(1);
  g1(1); g2(1); g3(1); g5(1); g6(1); g7(1);
  n1(1); n2(1); n3(1); n4(1); n5(1); n6(1); n7(1); n8(1); n9(1);
  m1(1); m2(1); m3(1); m4(1); m5(1); m6(1); m7(1); m8(1); m9(1); ma(1); mb(1);
}
