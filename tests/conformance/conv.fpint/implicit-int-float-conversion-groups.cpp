// [conv.fpint]: -Wimplicit-float-conversion (and -Wconversion) include
// -Wimplicit-int-float-conversion, which includes the constant form;
// -Werror=implicit-int-float-conversion makes both errors and a later
// -Wno-implicit-const-int-float-conversion silences only the constant one.
//@ verify
//@ args: -Wimplicit-float-conversion -Werror=implicit-int-float-conversion -Wno-implicit-const-int-float-conversion

int gi;
long long gll;
float f1 = gi; // expected-error {{implicit conversion from 'int' to 'float' may lose precision}}
double d1 = gll; // expected-error {{implicit conversion from 'long long' to 'double' may lose precision}}
float f2 = 16777217;
float f3 = gi & 0xfff;
