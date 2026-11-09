// [module.unit]/3-8: a primary interface, an interface partition it
// re-exports, an internal partition and an implementation unit; the
// importer sees p's and p:part's exports. Each partition, then the
// primary interface, is compiled once to a module file and an object;
// the implementation unit p-def.cpp and this importer are compiled
// against those module files, and the objects linked (spec/modules.md
// §11.7).
//@ expect: run
//@ std: c++20
//@ module: p:part=Inputs/p-part.cppm
//@ module: p:impl=Inputs/p-impl.cppm
//@ module: p=Inputs/p.cppm
//@ unit: Inputs/p-def.cpp
import p;

int main() { return part_value() + p_total() == 17 ? 0 : 1; }
