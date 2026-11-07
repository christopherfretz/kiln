// [dcl.attr.grammar]/4, /6: clang 22 also spells `no_sanitize` as
// `[[gnu::no_sanitize]]` (GCC 8's attribute); kiln knows it and ignores it
// without a diagnostic (WP-667; spec/sema.md §4.5). clang 19 does not:
// gnu-no-sanitize-clang19.cpp.
//@ requires: clang>=22
//@ expect: compile-ok
//@ strict-diags
//@ args: -Werror -Wunknown-attributes

[[gnu::no_sanitize("undefined")]] int f1();
[[gnu::__no_sanitize__("address", "undefined")]] int f2();
