// A C library header found through -isystem, as glibc's <math.h> declares
// `extern double lgamma_r(double, int*) __THROW;`.
extern "C" {
double sys_lgamma_r(double, int*) noexcept;
double sys_throw(double) throw();
double sys_noexcept_true(double) noexcept(true);
double sys_potentially_throwing(double) noexcept(false);
}
double sys_cxx(double) noexcept;

namespace std { using size_t = decltype(sizeof 0); struct nothrow_t {}; }
// A missing specification on a replaceable global allocation function is
// only a warning, and warnings in system headers are suppressed.
void* operator new(std::size_t, const std::nothrow_t&) noexcept;
void* operator new(std::size_t, const std::nothrow_t&);
