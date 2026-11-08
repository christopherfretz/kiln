// [conv.fpint]: -Wno-implicit-int-float-conversion silences the default-on
// constant form with its parent group.
//@ verify
//@ args: -Wno-implicit-int-float-conversion

float f2 = 16777217; // expected-no-diagnostics
