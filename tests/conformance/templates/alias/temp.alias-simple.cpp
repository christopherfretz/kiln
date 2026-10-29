//@ expect: ast
//@ oracle: clang
// [temp.alias]/2: a template-id naming an alias template is the substituted
// type-id.
template <class T> using Ptr = T *;
Ptr<int> p;
Ptr<const char> s;
