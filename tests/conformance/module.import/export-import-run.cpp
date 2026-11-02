// [module.import]/7, [module.interface]/2: `export import a;` in b's
// interface makes a's exported declarations visible to b's importers; a's
// non-exported declarations stay invisible. The interfaces are built
// in-process and compiled as extra inputs for their definitions.
//@ expect: run
//@ std: c++20
//@ args: %{test_dir}/Inputs/a.cppm %{test_dir}/Inputs/b.cppm
//@ args: -fmodule-file=a=%{test_dir}/Inputs/a.cppm -fmodule-file=b=%{test_dir}/Inputs/b.cppm
//@ skip(clang): clang needs a two-step --precompile build of the Inputs modules that the harness cannot drive
import b;

int main() { return fa() + fb() == 12 ? 0 : 1; }
