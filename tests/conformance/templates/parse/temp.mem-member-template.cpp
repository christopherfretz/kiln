//@ expect: ast
//@ oracle: clang
// [temp.mem]/1: member templates of an ordinary class
struct S {
  template <class T> void f(T);
  template <class T> int g(T t) { return 0; }
  template <class T> struct Inner { T x; };
};
