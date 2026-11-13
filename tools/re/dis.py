"""Disassemble a routine of SCORCH.EXE.  Usage: dis.py EXE SEG:OFF LEN
SEG uses the Ghidra numbering (load segment + 0x1000) the RE projects cite.
Borland FP-emulator interrupts are rewritten to real x87 opcodes before decoding:
  CD 34..3B  -> 9B D8..DF   (fwait + ESC)
  CD 3D      -> 90 9B       (nop + fwait)
  CD 3C xx   -> segment-override + ESC (xx: bits 7-6 = segment, low bits | 0xC0 = ESC)"""
import subprocess, sys, tempfile, os, re
exe = open(sys.argv[1], 'rb').read()
seg, off = (int(x, 16) for x in sys.argv[2].split(':'))
n = int(sys.argv[3], 0)
base = 0x6a00 + (seg - 0x1000) * 16
buf = bytearray(exe[base: base + off + n])
SEGPFX = {0: 0x26, 1: 0x2E, 2: 0x36, 3: 0x3E}   # ES, CS, SS, DS (Borland INT 3Ch convention)
def run():
    with tempfile.NamedTemporaryFile(delete=False) as f:
        f.write(buf); name = f.name
    out = subprocess.run(['objdump', '-D', '-b', 'binary', '-m', 'i8086', '-M', 'intel',
                          f'--start-address={off}', f'--stop-address={off+n}', name],
                         capture_output=True, text=True).stdout
    os.unlink(name); return out
for _ in range(400):
    out = run(); changed = False
    for m in re.finditer(r'^\s*([0-9a-f]+):\s+cd 3([4-9a-d])', out, re.M):
        a = int(m.group(1), 16); k = int(m.group(2), 16)
        if k <= 0xb: buf[a], buf[a+1] = 0x9B, 0xD8 + (k - 4)
        elif k == 0xd: buf[a], buf[a+1] = 0x90, 0x9B
        else:
            x = buf[a+2]; buf[a], buf[a+1], buf[a+2] = 0x9B, SEGPFX[x >> 6], (x & 0x3F) | 0xC0
        changed = True; break           # re-decode from scratch after each patch
    if not changed: break
for line in out.splitlines():
    m = re.match(r'^\s*([0-9a-f]+):\s+(.*)$', line)
    if m: print(f"{seg:04x}:{int(m.group(1),16):04x}  {m.group(2)}")
