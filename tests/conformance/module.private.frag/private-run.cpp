// [module.private.frag]/1-2: declarations in the private module fragment
// are not reachable from importers, but its definitions are the module's.
//@ expect: run
//@ std: c++20
//@ module: pv=Inputs/pv.cppm
import pv;

int main() { return pv_get() == 42 ? 0 : 1; }
