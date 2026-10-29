//@ expect: ast
//@ oracle: clang
// [temp.inst]/2: `sizeof` requires a complete type, so the specialization is
// implicitly instantiated.
template <class T> struct Pair { T first; T second; };
static_assert(sizeof(Pair<char>) == 2);
static_assert(sizeof(Pair<int>) == 8);
