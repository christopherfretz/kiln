// Driver codegen flags (spec/driver.md "Codegen flags"): -fstack-protector-strong puts the
// `sspstrong` function attribute on every function kiln defines -- ordinary
// functions, main, the global-init and TLS wrapper/init functions, the
// virtual thunks and noexcept terminate paths -- as clang
// (`CodeGenModule::SetLLVMFunctionAttributesForDefinition`).
//@ args: -fstack-protector-strong
int ext(int);
struct D { ~D(); };
void may();
void nt() noexcept { D d; may(); }
static int g = ext(1);
thread_local int tl = ext(2);
int use() { return tl; }
struct B { virtual int f(); }; struct B2 { virtual int g(); };
struct C : B, B2 { int g() override { return 1; } };
C c;
int main() { char buf[16]; buf[0] = 0; return buf[0]; }
