// spec/codegen.md "Bounded template names" (WP-1710):
// linkage-type-dag-10x10.cpp at `-g`. Spelled with their template
// arguments the debug names of `A9` and `B6` would be 5.8e10 and 3.7e8
// bytes (`A(k) = 10 A(k-1) + 26`); before WP-1710 the compile ran for
// minutes. A name over 64 KiB is the template's name alone (`Fooob`,
// `Food`) with its `templateParams` kept, so the compile takes about as
// long and as much memory as without `-g`. kiln only: clang needs over
// 10 GiB on the same input.
//@ expect: run
//@ args: -g
//@ mem<=256
//@ skip(clang): exponential debug names (over 10 GiB with -g); kiln only
//@ skip(gcc): exponential debug names; kiln only
#include "linkage-type-dag-10x10.cpp"
