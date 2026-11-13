// [dcl.attr.noreturn]/1: the first declaration of a function shall specify
// `[[noreturn]]` if any declaration does. An explicit specialization's
// first declaration is the one clang 19 and 22 form from the template, so
// `[[noreturn]]` on the specialization is diagnosed (the note names the
// specialization itself); a redeclaration repeating it is not.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:15 error "'noreturn' attribute does not appear on the first declaration"
//@ diag: 12:31 note "previous declaration is here"
template <class T> void nr2();
template <> [[noreturn]] void nr2<int>();
template <> [[noreturn]] void nr2<int>() { throw 1; }
