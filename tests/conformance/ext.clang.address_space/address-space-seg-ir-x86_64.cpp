// clang's x86 segment qualifiers (WP-1813): `__seg_gs` and `__seg_fs` are
// predefined as address_space(256) and address_space(257), so loads and
// stores through them are `ptr addrspace(256)` / `ptr addrspace(257)`.
//@ expect: ir
//@ args: --target=x86_64-unknown-linux-gnu -ftrivial-auto-var-init=uninitialized
int load(__seg_gs int *p) { return *p; }
int loadfs(__seg_fs int *p) { return *p; }
void store(__seg_gs int *p, int v) { *p = v; }
int glob(int i) { return ((__seg_fs const int *)0)[i]; }
