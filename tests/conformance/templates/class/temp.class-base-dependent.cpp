//@ expect: ast
//@ oracle: clang
// [temp.dep.type], [class.derived]: a dependent base class is substituted when
// the derived specialization is instantiated.
template <class T> struct Base { T b; };
template <class T> struct Derived : Base<T> { T d; };
static_assert(sizeof(Derived<int>) == 8);
