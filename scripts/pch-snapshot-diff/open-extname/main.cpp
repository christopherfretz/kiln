extern "C" int pfx_later();
int call() { return pfx_now() + pfx_later(); }
