int main_hidden() { return pfx_hidden(); }
#pragma GCC visibility pop
int main_default() { return pfx_default() + main_hidden(); }
