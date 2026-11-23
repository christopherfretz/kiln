// [dcl.link]/4, [basic.link]/4: a function or variable of an unnamed
// namespace first declared in an `extern "C"` context keeps external
// linkage and C language linkage, as in clang (GCC 16.1 gives it internal
// linkage); so does a block-scope `extern` declaration in an `extern "C"`
// function of an unnamed namespace, and a redeclaration in an unnamed
// namespace of a global `extern "C"` function is the same function. Their
// symbols are the unmangled C names with external linkage. Compared with
// clang's IR.
//@ expect: ir
namespace {
extern "C" void f1() {}
extern "C" int x1 = 3;
extern "C" void f2();
extern "C" int x2;
extern "C" void k() {
  extern int w;
  extern void m();
  w = 1;
  m();
}
} // namespace

extern "C" {
namespace {
void cf() {}
int cv;
} // namespace
}

extern "C" void f3();
namespace {
extern "C" void f3() {}
} // namespace

void use() {
  f1();
  f2();
  x1 = x2;
  k();
  cf();
  cv = 2;
  f3();
}
