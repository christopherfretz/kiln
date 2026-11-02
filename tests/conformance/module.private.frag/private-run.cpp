// [module.private.frag]/1-2: declarations in the private module fragment
// are not reachable from importers, but its definitions are the module's.
//@ expect: run
//@ std: c++20
//@ args: %{test_dir}/Inputs/pv.cppm -fmodule-file=pv=%{test_dir}/Inputs/pv.cppm
//@ skip(clang): clang needs a two-step --precompile build of Inputs/pv.cppm that the harness cannot drive
import pv;

int main() { return pv_get() == 42 ? 0 : 1; }
