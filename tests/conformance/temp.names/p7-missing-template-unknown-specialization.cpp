// [temp.names]/7, [basic.lookup.qual.general]/3, [temp.dep.type]/7: after
// `.`/`->` the name is a qualified name, not an unqualified one, so when
// its lookup finds nothing (the object is dependent and not the current
// instantiation, or the name is a member of an unknown specialization:
// not found in the current instantiation, which has a dependent base) the
// `<` is a less-than and `f<int>()` does not parse: clang's "use
// 'template' keyword to treat 'f' as a dependent template name". A data
// member of an unknown specialization followed by `<` stays a comparison,
// as does `t.a < c > d`. Found running GCC's C++ testsuite (WP-1530).
//@ expect: compile-fail
//@ strict-diags
//@ args: -ferror-limit=0
//@ diag: 21:15 error "use 'template' keyword to treat 'f' as a dependent template name"
//@ diag: 22:19 error "use 'template' keyword to treat 'g' as a dependent template name"
//@ diag: 23:16 error "use 'template' keyword to treat 'g' as a dependent template name"
//@ diag: 24:28 error "use 'template' keyword to treat 'g' as a dependent template name"
template <class T> struct Base { template <class U> int g(); int v; };
template <class T> struct D : Base<T> {
  int w;
  int h(T t) {
    int a = t.f<int>();
    int b = this->g<int>();
    int c = D::g<int>();
    int e = this->Base<T>::g<int>();
    return a + b + c + e;
  }
  bool cmp(T t, int c, int d) {
    return (t.v < 0) && (this->v < 0) && (t.a < c > d) && (this->w < c);
  }
};
