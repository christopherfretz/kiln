// [clang ext] The version list of `target_clones` on x86-64 (clang
// `checkTargetClonesAttrString`): an unknown feature or architecture or
// an empty version drops the attribute with a warning, a duplicate
// version and a list that mixes several string literals with commas in
// one of them are warnings. The location of a version inside a string is
// clang 22's (x86-target-clones-list-19.cpp: clang 19's). spec/sema.md
// "Function multiversioning" (WP-1745).
//@ expect: compile-ok
//@ requires: clang>=22
//@ args: --target=x86_64-unknown-linux-gnu
//@ strict-diags
//@ diag: 23:30 warning "mixing 'target_clones' specifier mechanisms is permitted for GCC compatibility; use a comma separated sequence of string literals, or a string literal containing a comma-separated list of versions"
//@ diag: 24:38 warning "version list contains duplicate entries"
//@ diag: 25:30 warning "unsupported 'bogus' in the 'target_clones' attribute string; 'target_clones' attribute ignored"
//@ diag: 26:30 warning "unsupported CPU 'bogus' in the 'target_clones' attribute string; 'target_clones' attribute ignored"
//@ diag: 27:30 warning "unsupported '' in the 'target_clones' attribute string; 'target_clones' attribute ignored"
//@ diag: 28:43 warning "version list contains duplicate entries"
//@ diag: 29:30 warning "unsupported '' in the 'target_clones' attribute string; 'target_clones' attribute ignored"
//@ diag: 30:41 warning "version list contains duplicate entries"
//@ diag: 31:35 warning "unsupported '' in the 'target_clones' attribute string; 'target_clones' attribute ignored"
//@ diag: 32:30 warning "mixing 'target_clones' specifier mechanisms is permitted for GCC compatibility; use a comma separated sequence of string literals, or a string literal containing a comma-separated list of versions"
//@ diag: 33:30 warning "unsupported 'cmov' in the 'target_clones' attribute string; 'target_clones' attribute ignored"
__attribute__((target_clones("avx2,default", "sse4.2"))) int t3();
__attribute__((target_clones("avx2", "avx2", "default"))) int t4();
__attribute__((target_clones("bogus", "default"))) int t5();
__attribute__((target_clones("arch=bogus", "default"))) int t6();
__attribute__((target_clones("", "default"))) int t7();
__attribute__((target_clones("default,avx2,avx2"))) int t8();
__attribute__((target_clones("default,"))) int t9();
__attribute__((target_clones("default", "default"))) int t10();
__attribute__((target_clones("avx2,,default"))) int t11();
__attribute__((target_clones("default", "avx2", "sse4.2,popcnt"))) int t12();
__attribute__((target_clones("cmov", "default"))) int t13();
__attribute__((target_clones("default", " avx2", "popcnt "))) int t14();
