// [stmt.switch]: -Wswitch-enum, -Wcovered-switch-default and -Wswitch-default
// on top of -Wswitch (WP-219)
//@ expect: compile-ok
//@ strict-diags
//@ args: -Wswitch-enum -Wcovered-switch-default -Wswitch-default -Wno-deprecated-declarations
//@ diag: 48:3 warning "'switch' missing 'default' label"
//@ diag: 48:11 warning "4 enumeration values not handled in switch: 'B', 'C', 'D'..."
//@ diag: 49:3 warning "'switch' missing 'default' label"
//@ diag: 49:11 warning "enumeration values 'C', 'D', and 'F' not handled in switch"
//@ diag: 50:3 warning "'switch' missing 'default' label"
//@ diag: 50:11 warning "enumeration values 'D' and 'F' not handled in switch"
//@ diag: 51:11 warning "4 enumeration values not explicitly handled in switch: 'B', 'C', 'D'..."
//@ diag: 52:63 warning "default label in switch which covers all enumeration values"
//@ diag: 53:3 warning "'switch' missing 'default' label"
//@ diag: 53:11 warning "enumeration values 'Y' and 'Z' not handled in switch"
//@ diag: 54:3 warning "'switch' missing 'default' label"
//@ diag: 54:11 warning "enumeration values 'G1' and 'G2' not handled in switch"
//@ diag: 55:3 warning "'switch' missing 'default' label"
//@ diag: 55:11 warning "4 enumeration values not handled in switch: 'B', 'C', 'D'..."
//@ diag: 56:3 warning "'switch' missing 'default' label"
//@ diag: 56:11 warning "4 enumeration values not handled in switch: 'B', 'C', 'D'..."
//@ diag: 57:3 warning "'switch' missing 'default' label"
//@ diag: 57:29 warning "case value not in enumerated type 'E'"
//@ diag: 58:3 warning "'switch' missing 'default' label"
//@ diag: 58:11 warning "enumeration value 'F' not handled in switch"
//@ diag: 59:3 warning "'switch' missing 'default' label"
//@ diag: 60:3 warning "'switch' missing 'default' label"
//@ diag: 61:21 warning "case value not in enumerated type 'E'"
//@ diag: 61:11 warning "5 enumeration values not explicitly handled in switch: 'A', 'B', 'C'..."
//@ diag: 62:3 warning "'switch' missing 'default' label"
//@ diag: 62:11 warning "no case matching constant switch condition '0'"
//@ diag: 63:3 warning "'switch' missing 'default' label"
//@ diag: 64:3 warning "'switch' missing 'default' label"
//@ diag: 64:27 warning "case value not in enumerated type 'E'"
//@ diag: 65:3 warning "'switch' missing 'default' label"
//@ diag: 65:11 warning "4 enumeration values not handled in switch: 'B', 'C', 'D'..."
//@ diag: 66:3 warning "'switch' missing 'default' label"
//@ diag: 66:11 warning "5 enumeration values not handled in switch: 'A', 'B', 'C'..."
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
