// [basic.start.main]/1: under -ffreestanding `main` is an ordinary
// function (clang's FunctionDecl::isMain is false): a non-`int` return
// type is fine, the name is mangled (`_Z4mainv`) and no implicit
// `return 0` is added. kiln asserted on the pointer return. Found running
// GCC's C++ testsuite (WP-1456).
//@ expect: ir
//@ args: -ffreestanding
void* main() {}
int caller() { return main() != nullptr; }
