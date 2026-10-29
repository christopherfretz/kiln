//@ expect: ast
//@ oracle: clang
// [temp.mem.class], [temp.inst]/3: a nested class of a class template is
// declared with the enclosing specialization; its definition is instantiated
// when it is required to be complete.
template <class T> struct Outer {
  struct Inner { T x; };
  Inner *in;
};
Outer<int> *o;
int sz = sizeof(Outer<long>::Inner);
