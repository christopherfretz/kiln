//@ expect: ast
extern "C" {
int c_abs(int);
double c_fabs(double);
}
extern "C++" int cpp_fn(int);
int user(int v) { return c_abs(v) + cpp_fn(v) + (int)c_fabs(1.5); }
