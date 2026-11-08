// [conv.fpint]: -Wimplicit-const-int-float-conversion is on by default and
// names the changed value; the non-constant form is off by default, also
// under -Wall -Wextra. A run-time behavior diagnostic: not in unevaluated
// operands, unreachable code or the initializer of a constexpr variable.
//@ verify
//@ args: -Wall -Wextra

int gi;
float f1 = gi;
float f2 = 16777217; // expected-warning {{implicit conversion from 'int' to 'float' changes value from 16777217 to 16777216}}
double d1 = 9007199254740993LL; // expected-warning {{implicit conversion from 'long long' to 'double' changes value from 9007199254740993 to 9007199254740992}}
constexpr float cf = 16777217;
float f4 = cf;
float f3 = sizeof(float(16777217));
float k(int i) {
  if (i)
    return 16777217; // expected-warning {{implicit conversion from 'int' to 'float' changes value from 16777217 to 16777216}}
  return i;
  return 16777219;
}
