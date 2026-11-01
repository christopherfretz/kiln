// [stmt.return]/4 + [stmt.switch]: -Wreturn-type follows clang's CFG: a switch
// covering every enumerator, or a constant condition matching a case, has no
// fall-through default edge (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 18:23 warning "enumeration value 'C' not handled in switch"
//@ diag: 18:66 warning "non-void function does not return a value in all control paths"
//@ diag: 19:81 warning "non-void function does not return a value in all control paths"
//@ diag: 21:20 warning "no case matching constant switch condition '3'"
//@ diag: 21:63 warning "non-void function does not return a value"
//@ diag: 26:51 warning "non-void function does not return a value in all control paths"
//@ diag: 29:70 warning "non-void function does not return a value in all control paths"
//@ diag: 33:61 warning "non-void function does not return a value"
enum E { A, B, C };
enum class S { X, Y };
int f1(E e) { switch (e) { case A: case B: case C: return 1; } }
int f2(S s) { switch (s) { case S::X: return 1; case S::Y: return 2; } }
int f3(E e) { switch (e) { case A: return 1; case B: return 2; } }
int f4(E e) { switch (e) { case A: return 1; case B: return 2; case C: break; } }
int f5() { switch (2) { case 1: return 1; case 2: return 2; } }
int f6() { switch (3) { case 1: return 1; case 2: return 2; } }
int f7() { switch (3) { case 1: return 1; default: return 2; } }
int f8(E e) { switch (e) { case A: case B: case C: return 1; } ; }
int f9(E e) { switch (e) { case A: case B: return 1; default: return 3; } }
int f10(E e) { switch (e) { case A: case B: case C: return 1; default: break; } }
int f11(int e) { switch (e) { case 0: return 1; } }
enum F : int { P = 1, Q = 1, R };
int f12(F e) { switch (e) { case P: return 1; case R: return 2; } }
int f13(E e) { switch ((int)e) { case A: case B: case C: return 1; } }
int f14(E e) { { switch (e) { case A: case B: case C: return 1; } } }
int f15(E e) { if (e) { switch (e) { case A: case B: case C: return 1; } } else return 2; }
int f16(E e) { while (true) { switch (e) { case A: return 1; case B: case C: break; } } }
int f17() { switch (2) { case 1: return 1; case 2: break; } }
int f18() { switch (2) { case 1: break; case 2: return 1; } }
int f19(int x) { switch (x) { case 1: return 1; default: return 2; } }
