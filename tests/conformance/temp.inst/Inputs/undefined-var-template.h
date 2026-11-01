// Pattern in a system header: -Wundefined-var-template is not reported.
template <class T> extern T sys_v;
template <class T> struct SysS { static int m; };
