// The written-reference table (spec/ast.md §18.4, stage 3), role
// `spec-target`: the template name of an explicit specialization
// ([temp.expl.spec]/1), a partial specialization
// ([temp.spec.partial.general]/1) and an explicit instantiation
// definition or declaration ([temp.explicit]/2) names the primary
// template, after the names in its template argument list. A class
// specialization's names are enclosed by the specialization, a function
// or variable specialization's by the declaration (§18.3). A template-id
// in an elaborated reference (`struct Traits<char> *`) is a `template`
// entry.
//@ written-refs: spec-targets.refs
namespace lib {
template <class T> struct Traits {
  static constexpr int size = 1;
};
template <class T> int measure(T) { return 0; }
template <class T> constexpr int width = 4;
} // namespace lib

struct Point {};

template <> struct lib::Traits<Point> {
  static constexpr int size = 2;
};
template <class T> struct lib::Traits<T *> {
  static constexpr int size = 3;
};
template struct lib::Traits<long>;
extern template struct lib::Traits<short>;

namespace lib {
template <> int measure<Point>(Point) { return 1; }
template <> int measure(char) { return 2; }
template int measure<long>(long);
template int measure(int *);
extern template int measure<short>(short);
template <> constexpr int width<Point> = 8;
template <class T> constexpr int width<T *> = 16;
} // namespace lib

// A member of a class template specialization: the explicit
// instantiation names the specialization's member ([temp.explicit]/3).
template <class T> struct Box {
  Box();
  void put(T);
  static int count;
};
template <class T> Box<T>::Box() {}
template <class T> void Box<T>::put(T) {}
template <class T> int Box<T>::count = 0;
template void Box<int>::put(int);
template Box<int>::Box();
template int Box<int>::count;

struct lib::Traits<char> *traits_ptr;
int total = lib::Traits<Point>::size + lib::width<Point>;
