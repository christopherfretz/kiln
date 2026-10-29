//@ expect: ast
//@ oracle: clang
// [temp.spec.general]/5, [temp.inst]: each distinct argument list names one
// specialization, instantiated once however often it is named.
template <class T> struct Box { T value; };
static_assert(sizeof(Box<int>) == sizeof(int));
static_assert(sizeof(Box<int>) == 4);
static_assert(sizeof(Box<signed int>) == 4);
