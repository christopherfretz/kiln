// [module.unit]/3-8, spec/modules.md §2 and §11.6: part A's single
// invocation. One kiln++ command compiles a primary interface, the
// interface partition it re-exports, an internal partition, an
// implementation unit and this importer, each `-fmodule-file=` naming a
// module's source, which kiln builds in-process where it is imported;
// the objects are linked. partitions-run.cpp is the same program built
// separately (and checked against clang).
//@ expect: run
//@ std: c++20
//@ args: %{test_dir}/Inputs/p.cppm %{test_dir}/Inputs/p-part.cppm %{test_dir}/Inputs/p-impl.cppm %{test_dir}/Inputs/p-def.cpp
//@ args: -fmodule-file=p=%{test_dir}/Inputs/p.cppm -fmodule-file=p:part=%{test_dir}/Inputs/p-part.cppm
//@ args: -fmodule-file=p:impl=%{test_dir}/Inputs/p-impl.cppm
//@ skip(clang): one invocation over module sources is kiln's part A driver form; clang needs module files (partitions-run.cpp)
import p;

int main() { return part_value() + p_total() == 17 ? 0 : 1; }
