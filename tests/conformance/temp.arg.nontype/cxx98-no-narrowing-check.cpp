// [temp.arg.nontype]: C++98 has no converted constant expressions, so a
// non-type template argument converts to the parameter's type without a
// narrowing check (clang checks narrowing since C++11). Found running GCC's
// C++ testsuite (WP-1467).
//@ std: c++98
//@ strict-diags
template <long long MIN> long long mod(long long l, long long r) { return l % r + MIN; }
// The value is out of range of `long long`; C++98 converts it without the
// C++11 narrowing check ([temp.arg.nontype]/5 in C++03).
template long long mod<-0x8000000000000000LL>(long long, long long);
