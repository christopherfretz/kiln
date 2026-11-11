// GNU transparent_union in C (WP-1287): kept only on a union definition
// (or a typedef of a complete union) whose fields all have the first
// field's size and at most its alignment; anything else is ignored with a
// warning, and an argument no member takes is an error.
//@ std: c17
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:33 warning "'transparent_union' attribute only applies to unions"
//@ diag: 23:38 warning "can only be applied to a union definition"
//@ diag: 24:22 warning "'transparent_union' attribute only applies to unions"
//@ diag: 25:21 warning "'transparent_union' attribute only applies to unions"
//@ diag: 26:29 warning "size of field 'i' (32 bits) does not match the size of the first field"
//@ diag: 26:22 note "size of first field is 8 bits"
//@ diag: 27:22 warning "transparent union definition must contain at least one field"
//@ diag: 28:54 warning "first field of a transparent union cannot have floating point type 'double'"
//@ diag: 32:27 error "passing 'long *' to parameter of incompatible type 'union N'"
//@ diag: 19:18 note "passing argument to parameter here"
union N { int *p; const char *s; } __attribute__((transparent_union));
void take(union N);
void ok(int *ip, char *cp) { take(ip); take(cp); take(0); }
union Inc;
typedef int notu __attribute__((transparent_union));
typedef union Inc INC __attribute__((transparent_union));
int v __attribute__((transparent_union));
enum __attribute__((transparent_union)) E { X };
typedef union { char c; int i; } __attribute__((transparent_union)) BAD;
union __attribute__((transparent_union)) Empty { };
union __attribute__((transparent_union)) Fl { double d; long long l; };
// Above: an incomplete union at its typedef, a field size
// mismatch, an empty union and a floating first field each drop the
// attribute.
void bad(long *lp) { take(lp); }
