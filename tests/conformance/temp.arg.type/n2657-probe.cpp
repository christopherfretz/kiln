// [temp.arg.type] (C++11, N2657): local and unnamed types as template
// arguments. WP-646 feature probe.
//@ expect: run
//@ std: c++11
//@ strict-diags
//@ stdout: 3 5 1
extern "C" int printf(const char*, ...);
template <class T> int get(T t) { return t.v; }
template <class T> struct Box { T t; };
enum { Anon = 1 };
template <class T> int val(T e) { return static_cast<int>(e); }
int main() {
  struct Local { int v; };
  Box<Local> b = {{3}};
  struct { int v; } unnamed = {5};
  printf("%d %d %d\n", get(b.t), get(unnamed), val(Anon));
}
