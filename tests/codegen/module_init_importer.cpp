// Itanium ABI §3.2.6, [basic.start.dynamic]/5: a translation unit that
// is not a module unit calls its imports' initializers, in import order
// and skipping those that do nothing, at the start of _GLOBAL__sub_I.
//@ module: mod_init_a=Inputs/mod_init_a.cppm
//@ module: mod_init_c=Inputs/mod_init_c.cppm
//@ module: mod_init_b=Inputs/mod_init_b.cppm
import mod_init_c;
import mod_init_b;
import mod_init_a;
int v = vb + va + fc();
int main() { return v; }
