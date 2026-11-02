// [stmt.switch]: -Wswitch (on by default): unhandled enumerators, values not in
// the enum, constant conditions; deprecated/maybe_unused enumerators exempt (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ diag: 30:11 warning "4 enumeration values not handled in switch: 'B', 'C', 'D'..."
//@ diag: 31:11 warning "enumeration values 'C', 'D', and 'F' not handled in switch"
//@ diag: 32:11 warning "enumeration values 'D' and 'F' not handled in switch"
//@ diag: 35:11 warning "enumeration values 'Y' and 'Z' not handled in switch"
//@ diag: 36:11 warning "enumeration values 'G1' and 'G2' not handled in switch"
//@ diag: 37:11 warning "4 enumeration values not handled in switch: 'B', 'C', 'D'..."
//@ diag: 38:11 warning "4 enumeration values not handled in switch: 'B', 'C', 'D'..."
//@ diag: 39:29 warning "case value not in enumerated type 'E'"
//@ diag: 40:11 warning "enumeration value 'F' not handled in switch"
//@ diag: 43:21 warning "case value not in enumerated type 'E'"
//@ diag: 44:11 warning "no case matching constant switch condition '0'"
//@ diag: 46:27 warning "case value not in enumerated type 'E'"
//@ diag: 47:11 warning "4 enumeration values not handled in switch: 'B', 'C', 'D'..."
//@ diag: 48:11 warning "5 enumeration values not handled in switch: 'A', 'B', 'C'..."
//@ requires: clang<22
// clang 22 emits warning: enumeration value 'H1' not handled in switch; WP-355 residual
enum E { A, B, C, D, F };
enum class S : unsigned char { X, Y, Z };
enum G { G0 = 3, G1 = 1, G2 = 2, G3 = 1 };
struct T { E e; };
enum Empty {};
enum [[deprecated]] Dep { DA };
enum H { H0, H1 [[deprecated]], H2 [[maybe_unused]] };
E ge();
void f(E e, S s, G g, T t, Empty em, H h, int i) {
  switch (e) { case A: break; }
  switch (e) { case A: case B: break; }
  switch (e) { case A: case B: case C: break; }
  switch (e) { case A: break; default: break; }
  switch (e) { case A: case B: case C: case D: case F: break; default: break; }
  switch (s) { case S::X: break; }
  switch (g) { case G0: break; }
  switch (t.e) { case A: break; }
  switch (ge()) { case A: break; }
  switch (e) { case A: case 7: case B: case C: case D: case F: break; }
  switch (e) { case A ... C: case D: break; }
  switch (em) { case 0: break; }
  switch (h) { case H0: break; }
  switch (e) { case 7: break; default: break; }
  switch (A) { case B: break; }
  switch (i) { case 1: break; }
  switch (e) { case A ... 9: break; }
  switch ((E)(e + 1)) { case A: break; }
  switch (e) { }
}
