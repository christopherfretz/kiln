//@ expect: ast
//@ oracle: clang
// [temp.concept]/1: concept definitions
template <class T> concept Always = true;
template <class T> concept Never = false;
template <class T, class U> concept Both = Always<T> && Always<U>;
template <class T> concept Either = Always<T> || Never<T>;
template <class T> concept Big = sizeof(T) > 4;
