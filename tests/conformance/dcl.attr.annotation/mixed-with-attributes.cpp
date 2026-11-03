//@ expect: compile-fail
//@ skip(clang): clang 19 does not implement P3394 annotations
//@ strict-diags
//@ diag: 11:14 error "an attribute-specifier cannot mix annotations and attributes"
//@ diag: 12:7 error "an attribute-specifier cannot mix annotations and attributes"
//@ diag: 13:14 error "an attribute-specifier cannot mix annotations and attributes"
// [dcl.attr.grammar]/1 (P3394R4): an attribute-specifier holds either an
// annotation-list or an attribute-list, and annotations take no
// attribute-using-prefix. Separate specifiers may be mixed.
[[nodiscard]] [[=1]] int ok();
[[nodiscard, =1]] int mixed_after();
[[=1, maybe_unused]] int mixed_before;
[[using gnu: =1]] int using_prefix;
[[=1]] [[maybe_unused]] [[=2]] int alternating;
