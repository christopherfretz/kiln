//@ expect: ast
//@ oracle: clang
// [expr.prim.req.nested]: nested requirements
template <class T> concept Small = sizeof(T) <= 8;
template <class T> concept C = requires (T t) { requires Small<T>; requires sizeof(t) > 0; };
