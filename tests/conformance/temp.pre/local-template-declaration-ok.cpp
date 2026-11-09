// [temp.pre]/2 valid twin: a generic lambda and a local class without
// member templates in a block are fine, and a template-declaration in a
// class member function's enclosing class scope is fine (WP-1063).
//@ expect: compile-ok
struct S {
  template<class T> struct Q { T t; };
  int g() {
    struct L { int m; };
    auto l = []<class T>(T x) { return x; };
    return l(L{1}.m) + Q<int>{2}.t;
  }
};
