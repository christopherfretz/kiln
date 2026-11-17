// [temp.inst]/11, [stmt.if]/2: a function template specialization named
// as a template argument in a template body is instantiated only when
// that body is: not for a template never instantiated, nor for a
// discarded branch of a constexpr if. Sibling of
// function-template-argument-instantiated.cpp.
//@ std: c++17
//@ expect: compile-ok
template <class T> void f(T) { T::fail; }
template <class T> void ok(T) {}
template <void (*P)(int)> struct ByPtr {};
template <int> void never() { ByPtr<f> a; (void)a; }
template <int N> void branch() {
  if constexpr (N != 0) {
    ByPtr<f> a;
    (void)a;
  } else {
    ByPtr<ok> b;
    (void)b;
  }
}
void use() { branch<0>(); }
