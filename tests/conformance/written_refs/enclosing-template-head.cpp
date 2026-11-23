// The written-reference table (spec/ast.md §18.3): a template's
// template-head ([temp.pre]/1) is held by its templated declaration,
// which holds the rest of the template's names: a default template
// argument and a non-type parameter's type ([temp.param]), a
// requires-clause ([temp.pre]/9), a template template parameter's own
// head, a member template's head in a class template ([temp.mem]), an
// alias, variable and class template, a concept. Without a template of
// the head's own (a partial specialization, [temp.spec.partial]; an
// out-of-line member of a class template, [temp.mem.func]) the head is
// held by the declaration it declares. A lambda in a default argument
// keeps its call operator.
//@ written-refs: enclosing-template-head.refs
namespace lib {
struct Shape {};
template <class T> struct Box {};
} // namespace lib

template <class T = lib::Shape> void make() {}
template <lib::Shape *P = nullptr> void point() {}
template <class T>
  requires(sizeof(lib::Shape) == 1)
void constrained() {}
template <template <class = lib::Shape> class TT> struct Takes {};
template <class T = lib::Shape> using Alias = lib::Box<T>;
template <class T = lib::Shape> int var = 0;
template <class T = lib::Shape> concept Any = true;
template <class T = decltype([] { return lib::Shape{}; }())> void lam() {}

template <class T = lib::Shape> struct Outer {
  template <class U = lib::Box<T>> void member();
  void plain();
};

template <class T>
  requires(sizeof(lib::Shape) == 1)
struct Spec {
  void out();
};
template <class T>
  requires(sizeof(lib::Shape) == 1)
void Spec<T>::out() {}

template <class T> struct Part {};
template <class T>
  requires(sizeof(lib::Shape) == 1)
struct Part<T *> {};
template <class T> int pvar = 0;
template <class T>
  requires(sizeof(lib::Shape) == 1)
int pvar<T *> = 1;
