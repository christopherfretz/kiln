// [stmt.switch]: -Wswitch-bool follows clang's isKnownToHaveBooleanValue; a
// bool switch covering both values does not fall off (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 18:18 warning "switch condition has boolean value"
//@ diag: 18:54 warning "non-void function does not return a value in all control paths"
//@ diag: 19:18 warning "switch condition has boolean value"
//@ diag: 19:76 warning "non-void function does not return a value in all control paths"
//@ diag: 20:18 warning "switch condition has boolean value"
//@ diag: 20:69 warning "non-void function does not return a value in all control paths"
//@ diag: 21:82 warning "non-void function does not return a value in all control paths"
//@ diag: 22:68 warning "non-void function does not return a value in all control paths"
//@ diag: 23:74 warning "non-void function does not return a value in all control paths"
//@ diag: 24:18 warning "switch condition has boolean value"
//@ diag: 24:57 warning "non-void function does not return a value in all control paths"
//@ diag: 25:25 warning "switch condition has boolean value"
//@ diag: 25:81 warning "non-void function does not return a value in all control paths"
int k1(bool b) { switch (b) { case true: return 1; } }
int k2(bool b) { switch (b) { case true: return 1; case false: return 0; } }
int k3(bool b) { switch (b) { case 1: return 1; case 0: return 0; } }
int k4(unsigned char c) { switch (c & 1) { case 1: return 1; case 0: return 0; } }
int k5(int c) { switch (c) { case 1: return 1; case 0: return 0; } }
int k6(bool b) { switch ((int)b) { case 1: return 1; case 0: return 0; } }
int k7(bool b) { switch (b) { case 0 ... 1: return 1; } }
int k8(bool b, int x) { switch (b && x) { case 1: return 1; case 0: return 0; } }
