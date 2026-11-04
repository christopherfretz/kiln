//@ args: -fvisibility-inlines-hidden
// -fvisibility-inlines-hidden (spec/codegen.md "Symbol visibility"; clang
// useInlineVisibilityHidden): an inline member function definition, and
// an implicit instantiation of a member function template, is `hidden`
// unless it carries an explicit visibility attribute. Free inline
// functions, the static locals of hidden inline functions and their
// guard variables keep the default visibility, as do explicit
// instantiation definitions.
struct S {
  int inl() { return 1; }
  int outl();
  inline int later();
  static int sinl() {
    static int n = 0;
    return ++n;
  }
  __attribute__((visibility("default"))) int kept() { return 4; }
  template <class T> T tm() { return T(); }
  virtual int vinl() { return 5; }
};
int S::outl() { return 2; }
int S::later() { return 3; }
inline int free_inline() {
  static int g = S::sinl();
  return g;
}
template <class T> struct TS {
  T m() { return T(); }
};
template struct TS<long>;
int use() {
  S s;
  TS<int> t;
  return s.inl() + s.outl() + s.later() + S::sinl() + s.kept() +
         s.tm<int>() + free_inline() + t.m() + s.vinl();
}
