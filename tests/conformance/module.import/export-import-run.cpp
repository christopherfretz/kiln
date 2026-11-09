// [module.import]/7, [module.interface]/2: `export import a;` in b's
// interface makes a's exported declarations visible to b's importers; a's
// non-exported declarations stay invisible. Each interface is compiled
// separately to a module file and an object, b against a's module file.
//@ expect: run
//@ std: c++20
//@ module: a=Inputs/a.cppm
//@ module: b=Inputs/b.cppm
import b;

int main() { return fa() + fb() == 12 ? 0 : 1; }
