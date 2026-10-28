//@ expect: ast
bool t = true;
bool f = false;
int *np = nullptr;
const int *cnp = nullptr;
bool test(int *p) { return p == nullptr || t != f; }
