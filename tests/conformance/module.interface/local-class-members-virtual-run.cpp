// [class.local], [class.virtual], [class.mfct]/1, [module.interface]: the
// virtual member functions, constructors and vtables of the local classes
// of an exported inline function are the module unit's (the classes are
// attached to the module, so their in-class members are not inline); the
// importer's copy of the function names them and the program links.
//@ expect: run
//@ std: c++26
//@ module: lcvirt=Inputs/local-class-virtual.cppm
import lcvirt;

int main() {
  if (lcvirt_call(5) != 6)
    return 1;
  return 0;
}
