//@ expect: ast
//@ oracle: clang
// [temp.arg.type]: a specialization is a type argument of another.
template <class T> struct Box { T value; };
static_assert(sizeof(Box<Box<short>>) == 2);
