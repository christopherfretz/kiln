//@ expect: ast
// [class.pre]/1: the class-virt-specifier `final`; `final` is an ordinary
// identifier elsewhere, including as a class name.
struct A final { int x; };
class B final {};
union U final { int i; float f; };
struct final { int y; };
