//@ expect: ast
//@ oracle: clang
// [temp.alias]/2: an alias template whose type-id uses another alias
// template.
template <class T> using Ptr = T *;
template <class T> using PtrPtr = Ptr<Ptr<T>>;
PtrPtr<int> pp;
