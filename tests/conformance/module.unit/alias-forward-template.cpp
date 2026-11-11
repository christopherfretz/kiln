// [temp.inst]/2, [module.interface]: an alias names a specialization
// of a class template before the template's definition; the later
// definition makes the specialization instantiable, in the module and
// in an importer; under `--bmi-image` the specialization's image record
// precedes the definition's and is completed at its first use after it
// (spec/modules.md §11.11, WP-1307).
//@ expect: run
//@ std: c++20
//@ module: aft=Inputs/alias-forward-template.cppm
import aft;
int main() { return aft::use() == 4 ? 0 : 1; }
