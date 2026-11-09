// [basic.start.main]/1: a freestanding implementation defines no rules
// for `main` (clang's isMain is false with -ffreestanding; WP-1015).
//@ std: c++26
//@ args: -ffreestanding
//@ expect: compile-ok
void main() {}
namespace n { int main(int); }
