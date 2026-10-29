//@ expect: ast
//@ oracle: clang
// [temp.inst]/3: implicit instantiation of a class template specialization
// instantiates the declarations, not the definitions, of its member functions.
template <class T> struct Box {
  T value;
  T get() const { return value; }
  void set(T v) { value = v; }
};
int size = sizeof(Box<int>);
