import sys
pid = sys.argv[1]
for line in open(f"/proc/{pid}/maps"):
    if not 'vdso' in line:
        continue
    _range, *_ = line.split()
    start, end = _range.split('-')
    start, end = int(start, 16), int(end, 16)

len_ = end-start
with open(f"/proc/{pid}/mem", "rb") as fd:
    fd.seek(start)
    data = fd.read(len_)

with open('dump.bin', 'wb') as fd:
    fd.write(data)
