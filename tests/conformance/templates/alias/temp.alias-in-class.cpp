//@ expect: ast
//@ oracle: clang
// [temp.alias], [temp.mem]: a member alias declaration of a class template is
// instantiated with the class.
template <class T> struct Holder {
  using pointer = T *;
  pointer p;
};
Holder<int> *h;
int sz = sizeof(Holder<double>);
