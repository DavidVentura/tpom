# TPOM
![Melting clock](/melting-clock.jpg?raw=true "Melting clock")

----

This library hijacks time-related functions in the vDSO ([1](https://man7.org/linux/man-pages/man7/vdso.7.html), [2](https://en.wikipedia.org/wiki/VDSO)) and allows replacing them with user-provided functions.


As an example, embedded into a Python project, via [py-tpom](https://github.com/davidventura/py-tpom):

```python
>>> import datetime
>>> datetime.datetime.now()
datetime.datetime(2022, 12, 1, 13, 47, 15, 574866)
>>> import tpom
>>> with tpom.Freeze(datetime.datetime(2012, 1, 14, 1, 2, 3)):
...     print(datetime.datetime.now())
datetime.datetime(2012, 1, 14, 1, 2, 3)
```

Inspired by [libfaketime](https://github.com/wolfcw/libfaketime), the main difference is no need to `LD_PRELOAD` any shared object (and to re-exec afterwards).

## How it works

To speed up frequently called syscalls, the kernel exposes them via the [vDSO](https://en.wikipedia.org/wiki/VDSO), which is a mechanism to map kernel functions to user memory space.  
The vDSO memory area contains an [ELF](https://en.wikipedia.org/wiki/Executable_and_Linkable_Format) blob, which can be scanned for dynamic symbols, such as:

```
DYNAMIC SYMBOL TABLE:
0000000000000c10  w   DF .text  0000000000000005  LINUX_2.6   clock_gettime
0000000000000bd0 g    DF .text  0000000000000005  LINUX_2.6   __vdso_gettimeofday
0000000000000c20  w   DF .text  0000000000000060  LINUX_2.6   clock_getres
0000000000000c20 g    DF .text  0000000000000060  LINUX_2.6   __vdso_clock_getres
0000000000000bd0  w   DF .text  0000000000000005  LINUX_2.6   gettimeofday
0000000000000be0 g    DF .text  0000000000000029  LINUX_2.6   __vdso_time
0000000000000be0  w   DF .text  0000000000000029  LINUX_2.6   time
0000000000000c10 g    DF .text  0000000000000005  LINUX_2.6   __vdso_clock_gettime
```

Each of these symbols is just a function that can be called.

This is *user-space* memory, each process has **full** control over it, and this is what TPOM does:

* Find the vDSO memory range by scanning /proc/self/maps
* Read the vDSO ELF blob
* Make the vDSO range writable
* Write a `jmp` as first instruction on each symbol's address, targetting a [trampoline](https://en.wikipedia.org/wiki/Trampoline_(computing)) which ends up calling the user-provided function

The vDSO before:

```
0000000000000c10 <__vdso_clock_gettime@@LINUX_2.6>:
 c10:	e9 9b fb ff ff       	jmp    7b0 <LINUX_2.6@@LINUX_2.6+0x7b0>
 c15:	66 66 2e 0f 1f 84 00 	data16 cs nop WORD PTR [rax+rax*1+0x0]
 c1c:	00 00 00 00 
```

the vDSO after:

```
0000000000000c10 <__vdso_clock_gettime@@LINUX_2.6>:
 c10:	48 b8 50 5b 7e 2c 37 	movabs rax,0x56372c7e5b50
 c17:	56 00 00 
 c1a:	ff e0                	jmp    rax
 c1c:	90                   	nop
 c1d:	90                   	nop
 c1e:	90                   	nop
 c1f:	90                   	nop
```

## Notes

* This **will not work** if your code executes syscalls directly.
* Only works on `x86_64` and `aarch64`, on Linux.
    * It can be extended by generating new opcodes and adding arch-specific vDSO symbol names (per [man 7 vdso](https://man7.org/linux/man-pages/man7/vdso.7.html))
* **No `LD_PRELOAD`**
