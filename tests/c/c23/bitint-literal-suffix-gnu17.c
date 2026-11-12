// WP-1331: before C23 the `wb`/`uwb` suffixes are a C23 extension
// (-Wc23-extensions, on by default), at the literal's start, in code and
// in #if, as clang 19/22.
//@ args: -std=gnu17
//@ strict-diags
//@ diag: 11:9 warning "'_BitInt' suffix for literals is a C23 extension"
//@ diag: 12:9 warning "'_BitInt' suffix for literals is a C23 extension"
//@ diag: 13:5 warning "'_BitInt' suffix for literals is a C23 extension"
//@ diag: 13:22 warning "'_BitInt' suffix for literals is a C23 extension"

int a = 1wb;
int b = 1uwb;
#if 1uwb - 2 > 0 && -1WB < 0
int c = 1;
#endif
