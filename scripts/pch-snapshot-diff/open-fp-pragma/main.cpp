double contract_off(double a, double b, double c) { return a * b + c; }
#pragma STDC FP_CONTRACT ON
double contract_on(double a, double b, double c) { return a * b + c; }
double use(double x) { return pfx_fma_on(x, x, x) + contract_off(x, x, x) + contract_on(x, x, x); }
