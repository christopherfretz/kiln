// Module of the module_init_* cases: a dynamically initialized variable.
export module mod_init_a;
int next() { static int n; return ++n; }
export int va = next();
