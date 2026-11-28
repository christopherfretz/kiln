// [class.local], [module.interface], spec/codegen.md §2a (WP-3687): local-class
// `constructor` / `used` member functions defined in an imported named-module
// unit are emitted only by the module's own object (so each constructor runs
// exactly once), while a template instantiated in the importer emits its own
// local-class `constructor` member there.
//@ expect: run
//@ std: c++20
//@ module: lcctor=Inputs/local-class-ctor-used.cppm
import lcctor;

int main() {
  t<int>();
  return (n_g == 1 && n_f == 1 && n_t == 1) ? 0 : 1;
}
