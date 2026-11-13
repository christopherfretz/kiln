// Ends with #pragma STDC FP_CONTRACT OFF in force.
inline double pfx_fma_on(double a, double b, double c) { return a * b + c; }
#pragma STDC FP_CONTRACT OFF
