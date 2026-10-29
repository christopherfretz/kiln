// [basic.namespace] + [namespace.def]: entities with the same name in
// different namespaces are distinct; nested namespace definitions extend
// the enclosing namespace.
//@ expect: run
//@ stdout: 1 2 3 4
extern "C" int printf(const char*, ...);

namespace a {
int value() { return 1; }
namespace b {
int value() { return 2; }
}
}  // namespace a
namespace a::b::c {
int value() { return 3; }
}
namespace a {
int more() { return b::value() + b::c::value() - 1; }
}

int main() { printf("%d %d %d %d\n", a::value(), a::b::value(), a::b::c::value(), a::more()); }
