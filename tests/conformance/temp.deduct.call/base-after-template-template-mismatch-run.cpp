// [temp.deduct.call]/4.3: when `P` is a simple-template-id and `A` does
// not match it, `P` is deduced against each base class of `A`, starting
// from the deductions made before `A` was tried. Matching `I<B>` against
// `ref<iface>` deduces `I = ref` before `B` fails against the template
// `iface`; the base `iface<root<iface>>` is still deduced with `I` free
// (a type-erasure library's interface casts; WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 7 8 9 1
extern "C" int printf(const char *, ...);

template <template <class> class I> struct root {};
template <class B> struct iface : B {
  int v = 7;
};
template <template <class> class I> struct ref : I<root<I>> {};

template <template <class> class I, class B> int f(I<B> &o) { return o.v; }
template <template <class> class I, class B> int g(const I<B> *o) { return o->v + 1; }
template <template <class> class I, class B> int h(I<B> &&o) { return o.v + 2; }

// `A` itself matches: no base is consulted.
template <class T> struct box {
  int v = 1;
};
template <template <class> class I, class B> int k(I<B> &o) { return o.v; }

int main() {
  ref<iface> r;
  box<int> b;
  printf("%d %d %d %d\n", f(r), g(&r), h(ref<iface>{}), k(b));
}
