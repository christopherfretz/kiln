//@ expect: ast
//@ oracle: clang
// [temp.mem]/1: member templates of a class template
template <class T> struct Outer {
  template <class U> U conv(T t);
  template <class U> struct In { T a; U b; };
};
