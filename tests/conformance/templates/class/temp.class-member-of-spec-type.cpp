//@ expect: ast
//@ oracle: clang
// [temp.inst]/3: a member whose type is another specialization requires that
// specialization to be complete when the class is instantiated.
template <class T> struct Inner { T v; };
template <class T> struct Outer { Inner<T> in; int tag; };
static_assert(sizeof(Outer<char>) == 8);
