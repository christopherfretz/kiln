//@ expect: ast
//@ oracle: clang
//@ requires: clang>=22
// [class.derived]: a non-dependent base of a class template is shared by every
// specialization.
struct Base { int b; };
template <class T> struct Derived : Base { T d; };
static_assert(sizeof(Derived<int>) == 8);
