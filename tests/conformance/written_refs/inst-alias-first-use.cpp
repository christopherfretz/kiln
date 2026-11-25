// Per-specialization entries (spec/ast.md §18.9.2, §18.9.3;
// spec/sema-templates.md §4.10): an alias template specialization is a
// substitution, not an instantiation ([temp.alias]/2), and kiln substitutes
// it once per argument list: a later use reuses the first substitution and
// its rows. Here a better match for the unqualified dependent call
// (`lib::make(X)`) is declared between the two uses of `R<lib::X>`, so
// argument-dependent lookup at the second use would find it; the program
// is then ill-formed, no diagnostic required ([temp.dep.candidate]/1).
// kiln keeps the first result, as GCC does (`b` is `int`, one row naming
// `lib::make(...)`); clang 19 and 22 substitute again and give `long`.
//@ oracle: gcc
//@ skip(clang): clang substitutes the alias again at each use, so `b` is `long` there
//@ written-refs: inst-alias-first-use.refs
namespace lib {
struct X {};
int make(...);
} // namespace lib

template <class T> using R = decltype(make(T()));
R<lib::X> a;

namespace lib {
long make(X);
} // namespace lib

R<lib::X> b;
static_assert(sizeof(a) == sizeof(int), "a is int");
static_assert(sizeof(b) == sizeof(int), "b is int, the first substitution's");
