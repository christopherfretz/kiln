// Module of the module_init_* cases: imports mod_init_a; its variable is
// initialized after mod_init_a's.
export module mod_init_b;
import mod_init_a;
export int vb = va + 1;
