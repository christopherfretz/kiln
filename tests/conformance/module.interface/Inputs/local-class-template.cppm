export module lctmpl;
// A local class of an exported inline function template: its members are
// instantiated, and emitted, by the importer that instantiates the
// function. An explicitly inline member of a local class is the
// importer's own copy too.
export template <class T> inline T lctmpl_twice(T x) {
  struct P {
    T a;
    T twice() const { return a + a; }
  };
  return P{x}.twice();
}
export template <class T> struct Box {
  T v;
  template <class U> U scaled(U u) const {
    struct S {
      U k;
      U by(T t) const { return k * U(t); }
    };
    return S{u}.by(v);
  }
};
export inline int lctmpl_inline(int x) {
  struct P {
    int a;
    inline int get() const { return a; }
  };
  return P{x}.get();
}
