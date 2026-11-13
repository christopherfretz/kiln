extern "C" int pfx_later() { return 2; }
int call() { return pfx_alias() + pfx_alias_later(); }
