extern "C" int pfx_later();
int call() { return (pfx_later ? pfx_later() : 0) + (pfx_now ? pfx_now() : 0); }
