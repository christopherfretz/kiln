//@ args: -fvisibility-inlines-hidden
// -fvisibility-inlines-hidden (spec/codegen.md "Symbol visibility and
// `dso_local`"; clang useInlineVisibilityHidden): the definition of an
// inline function, member or not, including an implicit instantiation and
// a constructor, is `hidden` unless it carries an explicit visibility
// attribute. The static locals of such a function and their guard
// variables keep the default visibility (clang getLVForLocalDecl), as do
// out-of-line non-inline members and explicit instantiation definitions.
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
