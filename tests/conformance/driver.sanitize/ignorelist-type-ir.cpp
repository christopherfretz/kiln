// `-fsanitize-ignorelist=` `type:` entries (clang 22): arithmetic whose
// type (as written, a typedef name or the builtin's spelling) the section
// for its overflow check names is not checked, and an implicit
// conversion to a type named under the truncation check of the
// destination's signedness skips its truncation and sign-change checks.
// spec/codegen.md "Sanitizers", WP-1032.
//@ expect: ir
//@ std: c++20
//@ requires: clang>=22
//@ args: -fsanitize=signed-integer-overflow,unsigned-integer-overflow,implicit-conversion -fsanitize-ignorelist=%{test_dir}/Inputs/ignorelist-type.txt
typedef int Wrapping;

Wrapping wadd(Wrapping a, Wrapping b) { return a + b; }
int iadd(int a, int b) { return a + b; }
unsigned uadd(unsigned a, unsigned b) { return a + b; }
unsigned long ladd(unsigned long a, unsigned long b) { return a + b; }
Wrapping wneg(Wrapping a) { return -a; }
void winc(Wrapping &a) { ++a; }
char to_char(int i) { return i; }
short to_short(int i) { return i; }
signed char to_schar(int i) { return i; }
unsigned char to_uchar(int i) { return i; }
unsigned short to_ushort(int i) { return i; }
unsigned to_unsigned(int i) { return i; }
