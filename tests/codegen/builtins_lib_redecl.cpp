// A library builtin and the C function it names share one LLVM function:
// `__builtin_strlen` declares `strlen` first (clang
// `getBuiltinLibFunction`), and the later direct call of `strlen` reuses
// that declaration instead of a uniqued `strlen.1` (clang
// `GetOrCreateLLVMFunction`; WP-286, libc++ `__constexpr_strlen` in
// `<format>`).
extern "C" unsigned long strlen(const char*);
unsigned long a(const char* p) { return __builtin_strlen(p); }
unsigned long b(const char* p) { return strlen(p); }
int main() { return a("ab") + b("abc"); }
