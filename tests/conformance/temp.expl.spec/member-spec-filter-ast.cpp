//@ expect: ast
//@ ast-filter: N
//@ xfail(kiln): the dump filter names a specialization context without its arguments (`n::X::N`), skips a template's pattern and visits an explicit instantiation's members (kiln_ast dump_filtered)
// [temp.expl.spec]/1, [temp.explicit]: a member of an implicit
// specialization explicitly specialized is named by the specialization,
// as is a member of an explicit (partial) specialization; the members of
// an explicit instantiation are not searched (clang -ast-dump-filter)
namespace n {
template <class T, bool B = false>
struct X {
  struct N { T a; };
};
}
template <>
struct n::X<int>::N { long z; };
template <>
struct n::X<char> { struct N { int y; }; };
template struct n::X<unsigned, true>;
template <class T>
struct n::X<T *, true> { struct N { T *p; }; };
