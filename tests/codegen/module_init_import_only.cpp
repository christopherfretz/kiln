// Itanium ABI §3.2.6: an importer with no initializers of its own still
// runs its imports' initializers from _GLOBAL__sub_I.
//@ module: mod_init_a=Inputs/mod_init_a.cppm
import mod_init_a;
int main() { return va; }
