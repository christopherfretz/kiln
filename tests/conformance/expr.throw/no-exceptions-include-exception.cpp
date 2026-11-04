// [expr.throw], [except.pre]: with -fno-exceptions a throw-expression in a
// system header is not diagnosed (clang `ActOnCXXThrow`), so the standard
// library's own uninstantiated `std::throw_with_nested` (libstdc++
// bits/nested_exception.h, a raw `throw`) does not reject every TU that
// includes <exception>. spec/sema.md "Exceptions disabled", WP-607.
//@ expect: compile-ok
//@ strict-diags
//@ args: -fno-exceptions
#include <exception>
#include <new>
#include <string>

std::string s = "x";
std::unique_ptr<int> p;
