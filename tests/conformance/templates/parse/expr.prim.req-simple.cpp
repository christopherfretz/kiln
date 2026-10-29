//@ expect: ast
//@ oracle: clang
// [expr.prim.req.simple]: simple requirements
template <class T> concept Addable = requires (T a, T b) { a + b; a.size(); };
template <class T> concept Derefable = requires (T p) { *p; p->x; };
