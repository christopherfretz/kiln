//@ expect: ast
//@ oracle: clang
// [temp.inst]/5: a member function's definition is instantiated when it is
// odr-used.
template <class T> struct Box {
  T value;
  T get() const { return value; }
  void set(T v) { value = v; }
};
int f(Box<int> &b) {
  b.set(3);
  return b.get();
}
