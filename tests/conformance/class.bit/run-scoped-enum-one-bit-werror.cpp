// [class.bit]/1, [dcl.enum]/5: a scoped enumeration is not a signed integer
// type, so storing `Av::Has` (1) into a one-bit `Av` bit-field is no
// -Wsingle-bit-bitfield-constant-conversion (clang's `isSignedIntegerType()`)
// and a -Wall -Werror build compiles; the shape of a downstream
// record header. The stored value reads back sign-extended from
// the underlying `int`, as clang's codegen does (-1, so not `Av::Has`).
//@ expect: run
//@ args: -Wall -Werror
//@ exit: 0
enum class Av { Waiting, Has };
struct R { Av a : 1; void set() { a = Av::Has; } };
int main() { R r; r.set(); return r.a == Av::Has; }
