// clang 22 removed the __is_referenceable trait: the name is an ordinary
// identifier again, and __has_builtin reports it absent (spec/sema.md
// §1.1; [lex.name] reserved identifiers are still declarable).
//@ strict-diags
//@ requires: clang>=22
static_assert(!__has_builtin(__is_referenceable));
template <class T> constexpr bool __is_referenceable = sizeof(T) != 1;
static_assert(__is_referenceable<int> && !__is_referenceable<char>);
