// C17 6.8.4.2p5: a case value is converted to the promoted type of the
// controlling expression. clang's `checkCaseValue` warns (-Wswitch) when
// the value, at its own width, does not survive a round trip through the
// unpromoted condition type; the warnings come when the switch ends, last
// case first (WP-1312, WP-1309 follow-up).
//@ strict-diags
//@ diag: 18:38 warning "overflow converting case value to switch condition type (-1 to 255)"
//@ diag: 18:21 warning "overflow converting case value to switch condition type (256 to 0)"
//@ diag: 19:40 warning "overflow converting case value to switch condition type (65535 to 4294967295)"
//@ diag: 19:21 warning "overflow converting case value to switch condition type (70000 to 4464)"
//@ diag: 20:21 warning "overflow converting case value to switch condition type (4294967297 to 1)"
//@ diag: 21:3 warning "switch condition has boolean value"
//@ diag: 21:21 warning "overflow converting case value to switch condition type (2 to 0)"
//@ diag: 22:21 warning "overflow converting case value to switch condition type (4294967297 to 1)"
struct B { unsigned x : 3; };
enum E { A = 1 };
void f(unsigned char c, short s, int i, _Bool b, enum E e, struct B bf) {
  switch (c) { case 256: break; case -1: break; case 1: break; }
  switch (s) { case 70000: break; case 65535u: break; case 7: break; }
  switch (i) { case 4294967297LL: break; case 4294967295u: break; }
  switch (b) { case 2: break; }
  switch (e) { case 4294967297LL: break; }
  switch (bf.x) { case 9: break; }
}
