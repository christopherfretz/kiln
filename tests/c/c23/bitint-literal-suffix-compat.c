// WP-1331: from C23 on the `wb`/`uwb` suffixes warn only under the
// off-by-default -Wpre-c23-compat, as clang 19/22.
//@ std: c23
//@ args: -Wpre-c23-compat
//@ strict-diags
//@ diag: 9:9 warning "'_BitInt' suffix for literals is incompatible with C standards before C23"
//@ diag: 10:5 warning "'_BitInt' suffix for literals is incompatible with C standards before C23"

int a = 1wb;
#if 1uwb
int b = 1;
#endif
