// The written-reference table (spec/ast.md §18.2): the leading `::` of
// a name whose qualifier is the global scope is kept beside its entry
// and dumped as `q=::L:C` (the `::` token's own location), and the `~`
// of a destructor name as `tilde=L:C`. A `::` spelled by a macro is at
// the macro's expansion; `:: template`, a member destructor after
// `->::`, a pseudo-destructor of a scalar, a destructor's qualified
// declarator-id, a typename-specifier, and a `::` cut from its name by
// a comment, a line break or a line splice (or split by one) each keep
// the `::` token's location.
//@ written-refs: global-colons.refs
//@ std: c++20
namespace a {
int f() { return 1; }
struct S {
  static int count;
  ~S();
  template <class T> static int g() { return sizeof(T); }
};
template <class T> struct A {
  using type = T;
  ~A() {}
};
} // namespace a
template <class T> struct G {
  static constexpr int x = sizeof(T);
};
typedef int I;
int top = 0;
#define GLOBAL ::
#define QUAL ::a::
int macros() { return GLOBAL a::f() + QUAL f() + GLOBAL top; }
int template_kw() { return ::template G<int>::x + ::a::S::template g<char>(); }
void dtors(a::S *p, a::A<int> *q, I *i) {
  p->::a::S::~S();
  q->::a::A<int>::~A();
  i->::I::~I();
}
::a::S::~S() {}
template <class T> int dep() { return sizeof(typename ::a::A<T>::type); }
int use_dep() { return dep<int>(); }
int spaced() {
  return :: /* a comment */ a::f() + ::
      top + ::\
a::S::count + :\
:top;
}
