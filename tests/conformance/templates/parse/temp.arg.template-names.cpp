//@ expect: ast
//@ oracle: clang
// [temp.arg.template]/1: template template arguments
template <template <class> class TT> struct X {};
template <template <class> class TT> struct Y { X<TT> x; };
template <class T> struct Z { X<T::template W> *p; };
