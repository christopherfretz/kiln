// [dcl.inline]/6, [basic.def.odr]/14, spec/modules.md §7 "Imported
// `used` functions" (WP-1832): the inline functions with GNU `used` an
// importer reaches only through the body of an imported function it
// odr-uses are defined `linkonce_odr` in a comdat there, as clang 22
// does when it reads that body; they link with the module's own copies
// into one function each. Under `--bmi-image` from the declaration
// image.
//@ expect: run
//@ std: c++20
//@ module: usedbody=Inputs/used-inline-body.cppm
//@ stdout: 7 3 5 1
import usedbody;
extern "C" int printf(const char *, ...);
static_assert(ce() == 5);

int main() { printf("%d %d %d %d\n", sum(), pick(3), ce(), addr() == mod_addr()); }
