// [conv.fpint]: -Werror=implicit-const-int-float-conversion makes the
// default-on constant form an error, enabling it again after -Wno-conversion
// turned off its parent groups; the non-constant form stays off.
//@ verify
//@ args: -Wno-conversion -Werror=implicit-const-int-float-conversion

int gi;
float f1 = gi;
float f2 = 16777217; // expected-error {{implicit conversion from 'int' to 'float' changes value from 16777217 to 16777216}}
double d1 = 18014398509481985LL; // expected-error {{implicit conversion from 'long long' to 'double' changes value from 18014398509481985 to 18014398509481984}}
