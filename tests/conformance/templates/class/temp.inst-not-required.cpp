//@ expect: ast
//@ oracle: clang
// [temp.inst]/2: a pointer or reference to a specialization does not require
// it to be complete; the specialization is declared but not instantiated.
template <class T> struct Box { T value; };
Box<int> *p = nullptr;
Box<char> &r = *(Box<char> *)nullptr;
