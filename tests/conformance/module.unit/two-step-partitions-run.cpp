// [module.unit]/3-8, spec/modules.md §11.7: partitions-run.cpp built
// separately. Each partition and the primary interface is compiled once
// to a module file and an object, the implementation unit p-def.cpp and
// this importer against those module files, and the objects linked.
//@ expect: run
//@ std: c++20
//@ module: p:part=Inputs/p-part.cppm
//@ module: p:impl=Inputs/p-impl.cppm
//@ module: p=Inputs/p.cppm
//@ unit: Inputs/p-def.cpp
import p;

int main() { return part_value() + p_total() == 17 ? 0 : 1; }
