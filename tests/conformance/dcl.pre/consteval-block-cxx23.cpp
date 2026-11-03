// [dcl.pre]/P3289R1: consteval blocks are C++26; before that `consteval {`
// is an error (the block is still parsed, so nothing else is reported).
//@ expect: compile-fail
//@ std: c++23
//@ skip(clang): clang 19 has no consteval blocks (P3289R1)
//@ strict-diags
//@ diag: 9:1 error "consteval blocks are only supported in C++26"
//@ diag: 10:12 error "consteval blocks are only supported in C++26"
consteval { static_assert(true); }
struct S { consteval {} };
consteval int ok() { return 0; }
