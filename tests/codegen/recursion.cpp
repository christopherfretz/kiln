// Direct and mutual recursion.
int fib(int n) { return n < 2 ? n : fib(n - 1) + fib(n - 2); }
bool is_odd(unsigned n);
bool is_even(unsigned n) { return n == 0 ? true : is_odd(n - 1); }
bool is_odd(unsigned n) { return n == 0 ? false : is_even(n - 1); }
long fact(long n) { if (n <= 1) return 1; return n * fact(n - 1); }
int recursion() { return fib(10) + is_even(10) + (int)fact(5); }
