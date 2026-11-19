// spec/codegen.md "Bounded template names" (WP-1710):
// linkage-pack-dag-36x17.cpp at `-g`. The debug name of `Q` spelled with
// its template arguments would be about 1.8e27 bytes (each level repeats
// the previous one 36 times, `L(k) = 36 L(k-1) + 76`), and the name of
// `ignore<P...>` as long; before WP-1710 the compile grew past 4 GB and
// failed an 8 GiB allocation. A name over 64 KiB is the template's name
// alone (`Foo`, `ignore`) with its `templateParams` kept, so the compile
// takes about as long and as much memory as without `-g`. kiln only:
// clang and GCC spend minutes on the same input.
//@ expect: run
//@ args: -g
//@ mem<=256
//@ skip(clang): exponential debug names (minutes with -g); kiln only
//@ skip(gcc): exponential debug names; kiln only
#include "linkage-pack-dag-36x17.cpp"
