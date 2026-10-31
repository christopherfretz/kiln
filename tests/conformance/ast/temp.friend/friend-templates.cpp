//@ expect: ast
// [temp.friend]/1: friend class and function templates of a class
// template, one declared before and one hidden.
template <class T> int peek(const T&);
template <class T> class Box {
  T v;
  template <class U> friend class Box;
  template <class U> friend int peek(const U&);
  template <class U> friend struct Reader;
};
class Key {
  int k;
  template <class T> friend int peek(const T&);
};
