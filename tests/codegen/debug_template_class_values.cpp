// Debug info (WP-1424): class-type template value arguments, the
// `value:` of each DITemplateValueParameter is clang's `emitAbstract`
// constant of the class's struct type: members (an int, a pointer, an
// array), bit-fields, an empty class (`undef`), a union with an active
// member (its literal struct) and an `auto` parameter deduced to a class
// type, in class and function templates. (A nested class member, a float
// member, a lambda and a parameter object odr-used in a function template
// differ from clang 19 in the specialization's name or the unit's
// retained types, not in the value: tests/conformance/temp.arg.nontype
// covers them at run time.)
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
template <P p> int fp() { return 1; }
XP<P{1, &gi, {3, 4}}> xp;
XB<B{1, 2}> xb;
XE<E{}> xe;
XU<U{.s = 1}> xu;
XA<In{5}> xa;
int use() {
  return xp.m + xb.m + xe.m + xu.m + xa.m + fp<P{1, &gi, {0, 0}}>();
}
