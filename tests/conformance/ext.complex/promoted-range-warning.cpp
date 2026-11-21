//@ requires: clang>=22
//@ strict-diags
//@ args: -fcomplex-arithmetic=promoted
//@ diag: 15:30 warning "excess precision is requested but the target does not support excess precision which may result in observable differences in complex division behavior, additional uses where the requested higher precision cannot be honored were found but not diagnosed"
// WP-1795: clang 22's -Whigher-precision-for-complex-division under
// -fcomplex-arithmetic=promoted: a complex divisor whose element has no
// wider type (`long double`) is recorded, and at the end of the
// translation unit one warning per wider type is given at the first such
// division, with the "additional uses" clause for a repeat. `float` and
// `double` divisors are promoted, and a real divisor is never complex
// division.
using CF = _Complex float;
using CD = _Complex double;
using CL = _Complex long double;
CL q1(CL a, CL b) { return a / b; }
CF q2(CF a, CF b) { return a / b; }
CD q3(CD a, CD b) { return a / b; }
CL q4(CL a, long double b) { return a / b; }
void q5(CL &a, CL b) { a /= b; }
template <class T> T q6(T a, T b) { return a / b; }
template CD q6(CD, CD);
