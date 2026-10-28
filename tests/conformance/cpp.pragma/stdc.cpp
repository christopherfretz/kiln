// [cpp.pragma]/2: STDC pragmas are reserved; the standard ones are accepted
// and an unknown one in the STDC namespace is diagnosed.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:14 warning "unknown pragma in STDC namespace"
#pragma STDC FP_CONTRACT ON
#pragma STDC FP_CONTRACT OFF
#pragma STDC FENV_ACCESS OFF
#pragma STDC CX_LIMITED_RANGE DEFAULT
#pragma STDC FROBNICATE ON
int x;
