// Driver codegen flags (spec/driver.md "Codegen flags"): with
// -fno-asynchronous-unwind-tables, -funwind-tables makes every defined
// function `uwtable(sync)` and the "uwtable" module flag 1, as clang
// (`CodeGenOptions::UnwindTables`, the toolchain's default being async).
//@ args: -fno-asynchronous-unwind-tables -funwind-tables
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
