// Debug info (WP-1424): class-type template value arguments, the
// `value:` of each DITemplateValueParameter is clang's `emitAbstract`
// constant of the class's struct type: members (an int, a pointer, an
// array), bit-fields, an empty class (`undef`), a union with an active
// member (its literal struct) and an `auto` parameter deduced to a class
// type, in class and function templates. A member of a template
// parameter object read in a function template (`p.a`, a folded member
// access) or the object bound to a reference adds no retained type
// (clang's `tryEmitAsConstant` describes only variables and enumerators;
// WP-1414). The names: debug_template_value_spelling.cpp.
//@ args: -g
struct In { int i; };
struct P { int a; int *p; int arr[2]; };
struct B { int x : 3; int y : 5; };
struct E {};
union U { int i; short s; };
int gi;
template <P p> struct XP { int m; };
template <B b> struct XB { int m; };
template <E e> struct XE { int m; };
template <U u> struct XU { int m; };
template <auto A> struct XA { int m; };
template <P p> int fp() { return p.a; }
template <P p> int gp() { const P &r = p; return r.a; }
XP<P{1, &gi, {3, 4}}> xp;
XB<B{1, 2}> xb;
XE<E{}> xe;
XU<U{.s = 1}> xu;
XA<In{5}> xa;
int use() {
  return xp.m + xb.m + xe.m + xu.m + xa.m + fp<P{1, &gi, {0, 0}}>() + gp<P{2, &gi, {0, 0}}>();
}
