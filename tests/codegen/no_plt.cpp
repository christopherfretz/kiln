// Driver codegen flags (spec/driver.md "Codegen flags"): -fno-plt marks
// every function declared but not defined in the TU `nonlazybind` (the
// runtime's __cxa_* / operator new/delete included) and adds the
// "RtLibUseGOT" module flag, as clang (`CodeGenModule::CreateRuntimeFunction`,
// `SetFunctionAttributes`).
//@ args: -fno-plt
extern int ext(int);
__attribute__((visibility("hidden"))) int hid(int);
struct V { virtual int v(); };
inline int inl(int x) { return x; }
static int stat(int x) { return x; }
int defd(int x) { return x; }
extern int gv;
void thrower() { throw 1; }
int g(V &v) { int *p = new int(3); int r = ext(*p) + hid(1) + defd(2) + v.v() + inl(3) + stat(4) + gv; delete p; return r; }
static int s_init = ext(7);
