//@ expect: ast
//@ oracle: clang
//@ requires: clang>=22
// [temp.inst]/2: naming a specialization through a pointer does not require
// it to be complete, so a self-referential template is fine.
template <class T> struct List {
  T head;
  List<T> *tail;
  List<T *> *deeper;
};
int sz = sizeof(List<int>);
