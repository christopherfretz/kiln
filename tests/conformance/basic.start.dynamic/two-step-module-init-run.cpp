// [basic.start.dynamic]/5, spec/modules.md §11.5: module-init-run.cpp
// built separately. Each interface unit's object owns its `_ZGIW<module>`
// initializer; the importer's object calls those of its imports, and
// every variable is still initialized exactly once, after those of the
// units it has an interface dependency on.
//@ expect: run
//@ std: c++20
//@ module: seq=Inputs/mi-seq.cppm
//@ module: a=Inputs/mi-a.cppm
//@ module: c=Inputs/mi-c.cppm
//@ module: b=Inputs/mi-b.cppm
//@ stdout: a<b 1 c<b 1 b<main 1 once 1
import b;
import a;
import seq;
extern "C" int printf(const char *, ...);
int order_main = seq_take();

int main() {
  printf("a<b %d c<b %d b<main %d once %d\n", order_a < order_b, order_c < order_b,
         order_b < order_main, seq_next == 4 && order_main == 4);
}
