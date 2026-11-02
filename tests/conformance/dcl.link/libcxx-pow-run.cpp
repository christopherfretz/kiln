// [dcl.link]/6, [c.math.pow]: libc++'s std::pow(int, double) promotes and
// reaches __builtin_pow (declaring the library function `pow`), while
// std::pow(double, double) is the C library's ::pow; both name the one C
// function `pow`, so the module declares it once and the program links
// (previously `undefined reference to pow.4`, libc++
// rand.dist.norm.f/eval.pass.cpp; WP-288). The inline chain d <- c <- b
// makes the builtin's declaration come first in kiln's emission order.
//@ args: -stdlib=libc++
//@ expect: run
//@ stdout: 8 9 0.25
#include <cmath>
#include <cstdio>

inline double a(double t) { return std::pow(2, t + 1); }  // __builtin_pow
inline double d(double t) { return std::pow(3.0, t); }    // ::pow
inline double e(double t) { return std::pow(t, -t); }     // ::pow
inline double c(double t) { return d(t); }
inline double b(double t) { return c(t); }

int main(int argc, char**) {
  double two = argc + 1;
  double x = a(two);
  double y = b(two);
  std::printf("%g %g %g\n", x, y, e(two));
}
