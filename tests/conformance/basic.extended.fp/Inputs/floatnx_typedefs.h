// glibc's <bits/floatn-common.h> for a C++ compiler that is not GCC >= 13
// (kiln reports __GNUC__ 4, as clang does).
typedef double _Float32x;
typedef long double _Float64x;
