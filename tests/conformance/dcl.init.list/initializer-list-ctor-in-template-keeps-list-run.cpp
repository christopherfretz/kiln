// [dcl.init.list]/3.7, /5, [over.match.list]/1.1, [temp.inst]: in a
// function template, `V v = {{"k1", "v1"}, {"k2", "v2"}}` calls the
// `std::initializer_list<P>` constructor with a two-element list, as it
// does outside a template. Instantiation must rebuild the braced list
// itself, not a list holding the `std::initializer_list` (`{{...}}`):
// that one-element list would initialize one `P` from the two inner
// lists, here through `S`'s iterator-pair constructor template. Reduced
// from clang's unittests/Analysis/FlowSensitive/ChromiumCheckModelTest.cpp
// (`const FileContentMappings FileContents = {{"check.h", H1}, ...}`).
//@ expect: run
//@ stdout: copy 2 1
//@ stdout: direct 2 1
//@ stdout: arg 2 1
//@ stdout: dependent 2 1
//@ stdout: extra 1 2
//@ stdout: plain 2 1
#include <cstdio>
#include <initializer_list>

struct S {
  int tag;
  S(const char *) : tag(1) {}
  template <class It> S(It, It) : tag(2) {}
};
struct P {
  S a, b;
  P(const S &x, const S &y) : a(x), b(y) {}
};
template <class E> struct V {
  int n, t;
  V(std::initializer_list<E> l) : n(int(l.size())), t(l.begin()->b.tag) {}
};

void show(const char *w, const V<P> &v) { std::printf("%s %d %d\n", w, v.n, v.t); }

template <class T> void run() {
  V<P> a = {{"k1", "v1"}, {"k2", "v2"}};
  show("copy", a);
  V<P> b{{"k1", "v1"}, {"k2", "v2"}};
  show("direct", b);
  show("arg", {{"k1", "v1"}, {"k2", "v2"}});
  V<T> d = {{"k1", "v1"}, {"k2", "v2"}};
  show("dependent", d);
  // One more brace level really is a one-element list.
  V<P> x = {{{"k1", "v1"}, {"k2", "v2"}}};
  show("extra", x);
}

int main() {
  run<P>();
  V<P> c = {{"k1", "v1"}, {"k2", "v2"}};
  show("plain", c);
}
