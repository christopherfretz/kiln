// [class.local], [module.interface]: an exported inline function's local
// class's member function, called by the importer's copy of the
// function, is emitted (found by WP-1350; the same with replay and with
// the declaration image).
//@ expect: run
//@ std: c++26
//@ module: lcm=Inputs/local-class-members.cppm
//@ xfail(kiln): WP-1350 follow-up, a local class's member functions in an imported module's inline function are emitted by neither the module unit nor the importer (undefined reference at link time)
import lcm;

int main() { return lcm_sum(1) == 3 ? 0 : 1; }
