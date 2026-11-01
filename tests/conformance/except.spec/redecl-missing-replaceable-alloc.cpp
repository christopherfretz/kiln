// [except.spec]/5, [new.delete]/2: a redeclaration of a replaceable global
// allocation function that omits the previous declaration's exception
// specification is accepted as an extension (clang's ExtWarn
// -Wmissing-exception-spec) and inherits it (WP-268).
//@ expect: compile-ok
//@ strict-diags
//@ diag: 11:7 warning "'operator new[]' is missing exception specification 'noexcept'"
//@ diag: 10:7 note "previous declaration is here"
namespace std { using size_t = decltype(sizeof 0); struct nothrow_t {}; }
void* operator new[](std::size_t, const std::nothrow_t&) noexcept;
void* operator new[](std::size_t, const std::nothrow_t&);
static_assert(noexcept(operator new[](1, std::nothrow_t{})));
