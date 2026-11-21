// [cpp.module], [cpp.import] are C++ only: in C a line starting with
// `import` or `module` is ordinary text (C17 6.4.2), here declarations
// with typedef names.
//@ std: c17
//@ strict-diags
typedef int import;
typedef int module;
import m;
module x;
int main(void) { return m + x; }
