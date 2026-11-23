// The declaration-extent table (spec/ast.md §19.1, §19.5): an
// elaborated-type-specifier that names a class or enumeration declared
// elsewhere declares nothing ([dcl.type.elab]) and has no row, also when
// it is qualified (`struct n::S *`) and when its qualifier names a class
// template specialization whose member it instantiates
// (`struct O<int>::S *`, which gave the instantiated member a row with
// the specifier's name and qualifier, WP-2629); a friend declaration
// naming a specialization ([class.friend]) adds none either, an
// implicit one (`friend struct n::V<int>;`) included. A specifier that
// declares the class has its row: `struct Fresh *` and `friend class
// Fr;` naming an undeclared class ([dcl.type.elab], [namespace.memdef]),
// and each `struct R;`, a redeclaration.
//@ decl-extents: elaborated-refs.extents
//@ std: c++26
namespace n {
struct S {};
enum E { e0 };
template <class> struct V {};
}
template <class T> struct O {
  struct S {};
  enum E { e1 };
};
struct n::S *p;
enum n::E e;
struct O<int>::S *q;
enum O<long>::E f;
struct F {
  friend struct n::S;
  friend struct O<char>::S;
  friend struct n::V<int>;
  friend class Fr;
};
struct Fresh *r;
struct R;
struct R;
