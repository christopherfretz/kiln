// [basic.fundamental]/1 (C23 6.2.5): `_BitInt(N)` is a clang extension in C++;
// `-pedantic` warns at the keyword (`-Wbit-int-extension`).
//@ args: -pedantic
//@ strict-diags
//@ diag: 7:1 warning "'_BitInt' in C++ is a Clang extension"
//@ diag: 8:10 warning "'_BitInt' in C++ is a Clang extension"
_BitInt(8) a;
unsigned _BitInt(3) b;
