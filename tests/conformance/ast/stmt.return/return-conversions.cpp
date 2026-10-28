//@ expect: ast
double widen(int i) { return i; }
int narrow(double d) { return d; }
bool truth(int *p) { return p; }
void nothing(int) { return; }
long chain(short s) { return s; }
unsigned char trunc(int i) { return i; }
