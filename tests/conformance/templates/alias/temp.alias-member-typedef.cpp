//@ expect: ast
//@ oracle: clang
// [temp.alias]/2: an alias template naming a member typedef of a
// specialization.
template <class T> struct Box { typedef T value_type; T v; };
template <class T> using ValueOf = typename Box<T>::value_type;
ValueOf<int> x;
