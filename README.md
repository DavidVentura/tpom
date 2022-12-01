# TPOM
![Melting clock](/melting-clock.jpg?raw=true "Melting clock")

----

This library hijacks time-related functions in the vDSO ([1](https://man7.org/linux/man-pages/man7/vdso.7.html), [2](https://en.wikipedia.org/wiki/VDSO)) and allows replacing them with user-provided functions.


As an example, embedded into a Python project:

```python
>>> import datetime
>>> datetime.datetime.now()
datetime.datetime(2022, 12, 1, 13, 47, 15, 574866)
>>> datetime.datetime.now()
datetime.datetime(2022, 12, 1, 13, 47, 16, 446386)
>>> import tpom
>>> tpom.curse_me()
>>> datetime.datetime.now()
my clockgettime was called!
datetime.datetime(1970, 1, 1, 1, 1, 51)
>>> datetime.datetime.now()
my clockgettime was called!
datetime.datetime(1970, 1, 1, 1, 1, 51)
```

Inspired by [libfaketime](https://github.com/wolfcw/libfaketime), the main difference is no need to `LD_PRELOAD` any shared object (and to re-exec afterwards).
