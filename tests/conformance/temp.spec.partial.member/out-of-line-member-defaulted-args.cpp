// [temp.spec.partial.general]/7, [temp.arg.general]/9: a declarator
// qualifier naming a partial specialization may leave arguments to the
// primary template's defaults; the partial's own arguments include
// them, so `E<W<U>>::I` names the member of `E<W<U>, typename
// W<U>::S>` (eigen SparseCwiseUnaryOp.h `unary_evaluator<CwiseUnaryOp<
// UnaryOp, ArgType>, IteratorBased>::InnerIterator`; WP-709).
//@ std: c++11
//@ expect: run
//@ strict-diags
//@ stdout: 7 3 2
extern "C" int printf(const char*, ...);
template <class Op> struct W { typedef int S; };
template <class T, class K = int, class S = typename T::S> struct E;
template <class U> struct E<W<U> > {
  class I;
  int f();
  static int g(typename E<W<U> >::I&);
};
template <class U> class E<W<U> >::I {
public:
  int v() const { return 7; }
};
template <class U> int E<W<U> >::f() { return 3; }
template <class U> int E<W<U> >::g(typename E<W<U> >::I&) { return 2; }
template <class U> struct E<W<U>, long> { class J; };
template <class U> class E<W<U>, long>::J {};
int main() {
  E<W<int> >::I i;
  E<W<int>, long>::J j;
  (void)j;
  printf("%d %d %d\n", i.v(), E<W<int> >().f(), E<W<int> >::g(i));
}
