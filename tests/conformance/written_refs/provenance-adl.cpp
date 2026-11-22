// Lookup provenance (spec/ast.md §18.7.4): argument-dependent lookup of
// a call's name ([basic.lookup.argdep]). `how=adl` when only it found
// the selected function, `how=both` when ordinary lookup found it too;
// `adl-ns=`/`adl-cls=`/`adl-enum=` list the associated set whenever the
// lookup was performed (empty for fundamental arguments), in discovery
// order. Each set below is derived from [basic.lookup.argdep]/3 by hand.
//@ written-refs: provenance-adl.refs
namespace n1 {
struct S {};
void take(S);
S operator+(S, S);
enum E { e0 };
void tag(E);
struct Outer {
  enum In { i0 };
};
void inner(Outer::In);
} // namespace n1
namespace n2 {
struct P {};
void apply(void (*)(P));
} // namespace n2
void handler(n2::P);
namespace n3 {
template <class T> struct Box {};
} // namespace n3
namespace n4 {
struct Q {};
template <class T> void peek(n3::Box<T>);
} // namespace n4
namespace n5 {
struct H {
  friend void poke(H) {}
};
} // namespace n5
namespace n6 {
struct T {};
void both(T);
} // namespace n6
using n6::both;
namespace n7 {
struct U {};
void viadir(U);
} // namespace n7
using namespace n7;
namespace n8 {
inline namespace v {
struct I {};
} // namespace v
void fi(I);
} // namespace n8
void plain(int);

struct M {
  void take(n1::S);
  // A class member found by ordinary lookup: no argument-dependent
  // lookup ([basic.lookup.argdep]/1), no set.
  void g(n1::S s) { take(s); }
};

void calls(n1::S s, n3::Box<n4::Q> bx, n5::H h, n6::T t, n7::U u, n8::I i) {
  // S is a class: its associated class is S, its namespace n1 (/3.2).
  take(s);
  // An enumeration: the enumeration and its innermost enclosing
  // namespace n1 (/3.3).
  tag(n1::e0);
  // A member enumeration: Outer (the class of which it is a member) is
  // reached first, adding n1, then the enumeration In (/3.3).
  inner(n1::Outer::i0);
  // A pointer to function: the entities of its parameter and return
  // types (/3.5 and /3.1 for `void`): the class P and namespace n2.
  apply(&handler);
  // A class template specialization: the class Box<Q> and n3 (/3.2),
  // then the entities of its template argument Q: the class Q and n4.
  peek(bx);
  // A hidden friend: H and n5; poke is found only because H is an
  // associated class (/4.2), so `how=adl`.
  poke(h);
  // Ordinary lookup finds n6::both through the using-declaration
  // (found= the shadow) and ADL through T finds it again: `both`.
  both(t);
  // Ordinary lookup finds n7::viadir through the file-scope
  // using-directive, ADL through U too: `both` with its directive.
  viadir(u);
  // An inline namespace: I's innermost namespace is the inline n8::v,
  // which adds its enclosing namespace n8 (/3, last sentences).
  fi(i);
  // Fundamental arguments associate nothing: an empty set.
  plain(1);
  // An operator-function-id written as a call is a call like any other:
  // S and n1; the operator expression `s + s` has no written name and
  // no entry (spec/ast.md §18.4).
  operator+(s, s);
  (void)(s + s);
}

namespace n9 {
enum { k0 };
template <class T> void unn(T) {}
} // namespace n9
// An unnamed enumeration: itself and n9 (/3.3); it prints as
// `(anonymous)` at its location. Only ADL finds unn: `how=adl`.
void unnamed() { unn(n9::k0); }
