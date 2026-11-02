// [module.unit]/3-8: a primary interface, an interface partition it
// re-exports, an internal partition and an implementation unit; the
// importer sees p's and p:part's exports.
//@ expect: run
//@ std: c++20
//@ args: %{test_dir}/Inputs/p.cppm %{test_dir}/Inputs/p-part.cppm %{test_dir}/Inputs/p-impl.cppm %{test_dir}/Inputs/p-def.cpp
//@ args: -fmodule-file=p=%{test_dir}/Inputs/p.cppm -fmodule-file=p:part=%{test_dir}/Inputs/p-part.cppm
//@ args: -fmodule-file=p:impl=%{test_dir}/Inputs/p-impl.cppm
//@ skip(clang): clang needs a two-step --precompile build of the Inputs modules that the harness cannot drive
import p;

int main() { return part_value() + p_total() == 17 ? 0 : 1; }
