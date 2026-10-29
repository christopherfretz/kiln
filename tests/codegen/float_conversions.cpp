// Float/double conversions and arithmetic.
float tof(double d) { return (float)d; }
double tod(float f) { return f; }
int toi(double d) { return (int)d; }
unsigned tou(float f) { return (unsigned)f; }
double fromu(unsigned long u) { return u; }
float fromi(int i) { return i; }
double arith(double a, float b) { return a / b - b * 2.5f + (a < b) + (a == b); }
double conv(double d) { return tof(d) + tod(1.0f) + toi(d) + tou(2.0f) + fromu(5) + fromi(-3) + arith(d, 2.0f); }
