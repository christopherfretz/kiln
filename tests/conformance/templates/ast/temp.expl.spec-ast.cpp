//@ expect: ast
//@ oracle: clang
// [temp.expl.spec]/1, [temp.explicit]/1: explicit specialization and
// explicit instantiation of a class template.
template <class T> struct E { T v; };
template <> struct E<int> { long w; };
template struct E<char>;
extern template struct E<short>;
E<int> *e;
int s = sizeof(E<int>);
