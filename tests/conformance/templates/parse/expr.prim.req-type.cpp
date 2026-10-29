//@ expect: ast
//@ oracle: clang
// [expr.prim.req.type]: type requirements
template <class T> concept HasType = requires { typename T::type; };
template <class T> concept HasMore = requires { typename T::value_type; typename T::template rebind<int>; };
