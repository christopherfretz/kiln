// [dcl.link]/8: the declaration directly contained in a braceless
// linkage-specification is not a definition, so its type may be incomplete
// (an array of unknown bound, as Intel's decimal library's
// `BID_EXTERN_C BID_UINT64 bid_round_const_table[][19];`), and a
// reference or `const` object needs no initializer.
//@ strict-diags
struct S;
extern "C" unsigned long long table[][19];
extern "C" S s;
extern "C" int &r;
extern "C" const int k;
extern "C" unsigned long long table[2][19] = {};
unsigned long long f() { return table[1][18]; }
