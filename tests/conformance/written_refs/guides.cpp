// The written-reference table (spec/ast.md §18.4, stage 3; WP-2100): a
// deduction guide's own name ([temp.deduct.guide]/1) names the class
// template, role `template`, as does the template-id after `->`. A
// guide declared in a class (a member class template's) too.
//@ written-refs: guides.refs
namespace box {
template <class T> struct Pair {
  Pair(T, T);
};
} // namespace box
namespace box {
template <class T> Pair(T, T) -> Pair<T>;
Pair(const char *, const char *) -> Pair<int>;
} // namespace box

struct Outer {
  template <class T> struct Cell {
    Cell(T);
  };
  template <class T> Cell(T *) -> Cell<T>;
};

box::Pair p{1, 2};
