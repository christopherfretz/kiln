//@ expect: ast
//@ oracle: clang
// [temp.alias]/2, [dcl.type.cv]: cv-qualifiers applied through an alias
// template.
template <class T> using C = const T;
template <class T> using Ref = const T &;
C<int> ci = 1;
Ref<int> r = ci;
