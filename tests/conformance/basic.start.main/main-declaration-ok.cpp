// [basic.start.main]/2-3 valid neighbours (WP-1015): `main` with typedef'd
// parameter types, an array parameter, const-qualified `char**` forms (a
// clang extension), redeclared with the same signature; functions and
// variables named `main` in namespaces, classes and blocks.
//@ std: c++26
//@ expect: compile-ok
typedef int I;
typedef char** CPP;
int main(I argc, CPP argv);
int main(int argc, char* argv[]) { return argc + (argv != nullptr); }
namespace N { int main(); int main(int); void f() { int main = 0; (void)main; } }
namespace V { int main; }
struct S { static int main; int main2; };
struct S2 { int main(float); };
int f() { int main = 1; static int m = 2; return main + m; }
namespace L { void g() { extern int main; } }
template<class T> struct TT { static T main; };
