// [basic.start.dynamic]/5, [module.import]/5: a variable of a module unit
// is initialized after those of the units it has an interface dependency
// on (b imports a and c), and the importing translation unit's variables
// after b's. Itanium ABI §3.2.6: each interface unit defines a guarded
// `_ZGIW<module>` initializer that runs its imports' initializers first,
// so every variable is initialized exactly once even though a's
// initializer is reached from b's, from main's and from its own
// llvm.global_ctors entry.
//@ expect: run
//@ std: c++20
//@ args: %{test_dir}/Inputs/mi-seq.cppm %{test_dir}/Inputs/mi-a.cppm %{test_dir}/Inputs/mi-c.cppm %{test_dir}/Inputs/mi-b.cppm
//@ args: -fmodule-file=seq=%{test_dir}/Inputs/mi-seq.cppm -fmodule-file=a=%{test_dir}/Inputs/mi-a.cppm
//@ args: -fmodule-file=c=%{test_dir}/Inputs/mi-c.cppm -fmodule-file=b=%{test_dir}/Inputs/mi-b.cppm
//@ stdout: a<b 1 c<b 1 b<main 1 once 1
//@ skip(clang): clang needs a two-step --precompile build of the Inputs modules that the harness cannot drive
import b;
import a;
import seq;
extern "C" int printf(const char *, ...);

int order_main = seq_take();

int main() {
  printf("a<b %d c<b %d b<main %d once %d\n", order_a < order_b, order_c < order_b,
         order_b < order_main, seq_next == 4 && order_main == 4);
}
