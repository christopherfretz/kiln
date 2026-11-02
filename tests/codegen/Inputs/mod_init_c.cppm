// Module of the module_init_* cases: functions only, so its initializer
// does nothing and importers do not call it.
export module mod_init_c;
export int fc() { return 3; }
