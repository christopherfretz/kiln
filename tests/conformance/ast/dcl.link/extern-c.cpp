//@ expect: ast
extern "C" int c_func(int);
extern "C" {
int c_var;
void c_other();
}
extern "C++" int cxx_func();
int use() { return c_func(c_var) + cxx_func(); }
