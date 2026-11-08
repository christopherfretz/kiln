// [dcl.attr] GNU `used` keeps an otherwise unreferenced static entity:
// the variables below are referenced only through the linker's
// `__start_`/`__stop_` symbols of their section, and the function only by
// name from the table, so without `used` none would be emitted (clang
// `UsedAttr`: `DeclMustBeEmitted` and `@llvm.used`; spec/landed/sema/WP-805.md).
// WP-805.
//@ expect: run
//@ strict-diags
//@ stdout: 3 entries, sum 60, hook 7
extern "C" int printf(const char *, ...);

struct Entry {
  int value;
};

__attribute__((used, section("kiln_reg"))) static const Entry e1 = {10};
__attribute__((used, section("kiln_reg"))) static const Entry e2 = {20};
namespace {
__attribute__((used, section("kiln_reg"))) const Entry e3 = {30};
}

extern "C" const Entry __start_kiln_reg[];
extern "C" const Entry __stop_kiln_reg[];

__attribute__((used)) static int hook() { return 7; }
extern "C" int kiln_used_hook() __asm__("_ZL4hookv");

int main() {
  int n = 0, sum = 0;
  for (const Entry *p = __start_kiln_reg; p != __stop_kiln_reg; ++p) {
    ++n;
    sum += p->value;
  }
  printf("%d entries, sum %d, hook %d\n", n, sum, kiln_used_hook());
}
