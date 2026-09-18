#!/usr/bin/env python3
from __future__ import annotations
import hashlib
import struct
import sys
from pathlib import Path

MENU = (
    b"QEVARYNOX-A11Y-UEFI-MENU-V1\r\n"
    b"STATE=UEFI_NATIVE_MENU\r\n"
    b"ITEM=1;ID=CONTINUE;LABEL=Continue boot\r\n"
    b"ITEM=2;ID=ACCESSIBILITY;LABEL=Accessibility help\r\n"
    b"ITEM=3;ID=RECOVERY;LABEL=Recovery status\r\n"
    b"PROMPT=PRESS_1_2_3\r\n"
    b"VISION_REQUIRED=0\r\n"
    b"POINTER_REQUIRED=0\r\n"
    b"KEYBOARD_REQUIRED=1\r\n"
    b"CHANNEL=NATIVE_COM1\r\n"
    b"SEMANTIC_SOURCE=UEFI_NATIVE_MENU_STATE\r\n"
    b"END\r\n"
)
HELP = (
    b"QEVARYNOX-A11Y-UEFI-MENU-V1\r\n"
    b"EVENT=SELECT\r\n"
    b"ID=ACCESSIBILITY\r\n"
    b"STATUS=READY\r\n"
    b"HELP=Press 1 to continue boot. Press 2 for accessibility help. Press 3 for recovery status.\r\n"
    b"VISION_REQUIRED=0\r\n"
    b"POINTER_REQUIRED=0\r\n"
    b"END\r\n"
)
RECOVERY = (
    b"QEVARYNOX-A11Y-UEFI-MENU-V1\r\n"
    b"EVENT=SELECT\r\n"
    b"ID=RECOVERY\r\n"
    b"STATUS=BLOCKED\r\n"
    b"REASON=RECOVERY_NOT_ESTABLISHED\r\n"
    b"ACTION_REQUIRED=0\r\n"
    b"VISION_REQUIRED=0\r\n"
    b"POINTER_REQUIRED=0\r\n"
    b"END\r\n"
)
INVALID = (
    b"QEVARYNOX-A11Y-UEFI-MENU-V1\r\n"
    b"EVENT=INVALID_KEY\r\n"
    b"STATUS=BLOCKED\r\n"
    b"REASON=KEY_NOT_MAPPED\r\n"
    b"PROMPT=PRESS_1_2_3\r\n"
    b"END\r\n"
)
CONTINUE = (
    b"QEVARYNOX-A11Y-UEFI-MENU-V1\r\n"
    b"EVENT=SELECT\r\n"
    b"ID=CONTINUE\r\n"
    b"STATUS=CONFIRMED\r\n"
    b"ACTION=RETURN_TO_FIRMWARE_BOOT_FLOW\r\n"
    b"VISION_REQUIRED=0\r\n"
    b"POINTER_REQUIRED=0\r\n"
    b"END\r\n"
)

class Code:
    def __init__(self):
        self.data=bytearray()
        self.labels={}
        self.fixups=[]

    def pos(self): return len(self.data)
    def emit(self,b): self.data += bytes(b)
    def label(self,name): self.labels[name]=self.pos()

    def rel32(self, opcode, label):
        self.emit(opcode)
        off=self.pos()
        self.emit(b"\x00"*4)
        self.fixups.append((off,self.pos(),label,4))

    def rel8(self, opcode, label):
        self.emit(bytes((opcode,0)))
        self.fixups.append((self.pos()-1,self.pos(),label,1))

    def lea_rsi(self,label):
        self.emit(b"\x48\x8d\x35")
        off=self.pos(); self.emit(b"\x00"*4)
        self.fixups.append((off,self.pos(),label,4))

    def lea_rdx(self,label):
        self.emit(b"\x48\x8d\x15")
        off=self.pos(); self.emit(b"\x00"*4)
        self.fixups.append((off,self.pos(),label,4))

    def patch(self):
        for off,after,label,width in self.fixups:
            disp=self.labels[label]-after
            if width==4:
                struct.pack_into("<i",self.data,off,disp)
            else:
                if not -128 <= disp <= 127:
                    raise SystemExit(f"short branch overflow for {label}: {disp}")
                self.data[off]=disp & 0xff

def build_code():
    c=Code()
    # EFI x64 entry: RCX=ImageHandle, RDX=SystemTable.
    c.emit(b"\x49\x89\xcc")              # mov r12,rcx
    c.emit(b"\x49\x89\xd5")              # mov r13,rdx
    c.emit(b"\x4c\x8b\x72\x30")          # mov r14,[rdx+0x30] ConIn
    c.emit(b"\x48\x83\xec\x28")          # Win64 shadow space + alignment

    # COM1 38400 8N1.
    for port,val in ((0x3F9,0x00),(0x3FB,0x80),(0x3F8,0x03),(0x3F9,0x00),(0x3FB,0x03),(0x3FA,0xC7),(0x3FC,0x0B)):
        c.emit(b"\x66\xba"+struct.pack("<H",port)+b"\xb0"+bytes((val,))+b"\xee")

    def emit_frame(label,data):
        c.lea_rsi(label)
        c.emit(b"\xb9"+struct.pack("<I",len(data)))
        c.rel32(b"\xe8","emit")

    c.label("menu")
    emit_frame("menu_frame",MENU)

    c.label("read")
    c.emit(b"\x4c\x89\xf1")               # mov rcx,r14
    c.lea_rdx("keybuf")                      # rdx=&EFI_INPUT_KEY
    c.emit(b"\x49\x8b\x46\x08")          # mov rax,[r14+8] ReadKeyStroke
    c.emit(b"\xff\xd0")                     # call rax
    c.emit(b"\x48\x85\xc0")               # test rax,rax
    c.rel32(b"\x0f\x85","read")             # retry EFI_NOT_READY / any nonzero
    c.lea_rsi("keybuf")
    c.emit(b"\x66\x8b\x46\x02")          # mov ax,[rsi+2] UnicodeChar
    c.emit(b"\x66\x3d\x31\x00")          # cmp ax,'1'
    c.rel32(b"\x0f\x84","continue")
    c.emit(b"\x66\x3d\x32\x00")          # cmp ax,'2'
    c.rel32(b"\x0f\x84","help")
    c.emit(b"\x66\x3d\x33\x00")          # cmp ax,'3'
    c.rel32(b"\x0f\x84","recovery")

    c.label("invalid")
    emit_frame("invalid_frame",INVALID)
    c.rel32(b"\xe9","menu")

    c.label("help")
    emit_frame("help_frame",HELP)
    c.rel32(b"\xe9","menu")

    c.label("recovery")
    emit_frame("recovery_frame",RECOVERY)
    c.rel32(b"\xe9","menu")

    c.label("continue")
    emit_frame("continue_frame",CONTINUE)
    c.emit(b"\x31\xc0")                     # EFI_SUCCESS
    c.emit(b"\x48\x83\xc4\x28")
    c.emit(b"\xc3")

    c.label("emit")
    c.emit(b"\x66\xba\xfd\x03")          # LSR
    c.label("emit_wait")
    c.emit(b"\xec\xa8\x20")
    c.rel8(0x74,"emit_wait")
    c.emit(b"\x66\xba\xf8\x03")          # THR
    c.emit(b"\x8a\x06\xee")               # mov al,[rsi]; out dx,al
    c.emit(b"\x48\xff\xc6")               # inc rsi
    c.emit(b"\xff\xc9")                     # dec ecx
    c.rel8(0x75,"emit_wait")
    c.emit(b"\xc3")

    c.label("keybuf")
    c.emit(b"\x00\x00\x00\x00")
    c.label("menu_frame"); c.emit(MENU)
    c.label("help_frame"); c.emit(HELP)
    c.label("recovery_frame"); c.emit(RECOVERY)
    c.label("invalid_frame"); c.emit(INVALID)
    c.label("continue_frame"); c.emit(CONTINUE)
    c.patch()
    return bytes(c.data),c.labels

def put(buf,off,fmt,*values):
    struct.pack_into(fmt,buf,off,*values)

def build_image():
    code,labels=build_code()
    raw_size=((len(code)+0x1ff)//0x200)*0x200
    text_raw=0x200
    reloc_raw=text_raw+raw_size
    file_size=reloc_raw+0x200
    text_rva=0x1000
    reloc_rva=((text_rva+len(code)+0xfff)//0x1000)*0x1000
    image_size=reloc_rva+0x1000

    image=bytearray(file_size)
    put(image,0x00,"<H",0x5A4D)
    put(image,0x3C,"<I",0x80)
    pe=0x80
    image[pe:pe+4]=b"PE\0\0"
    coff=pe+4
    put(image,coff,"<HHIIIHH",0x8664,2,0,0,0,0xF0,0x0022)
    opt=coff+20
    put(image,opt+0x00,"<H",0x20B)
    put(image,opt+0x04,"<I",raw_size)
    put(image,opt+0x08,"<I",0x200)
    put(image,opt+0x10,"<I",text_rva)
    put(image,opt+0x14,"<I",text_rva)
    put(image,opt+0x18,"<Q",0x400000)
    put(image,opt+0x20,"<I",0x1000)
    put(image,opt+0x24,"<I",0x200)
    put(image,opt+0x38,"<I",image_size)
    put(image,opt+0x3C,"<I",0x200)
    put(image,opt+0x44,"<H",10)
    put(image,opt+0x48,"<Q",0x100000)
    put(image,opt+0x50,"<Q",0x1000)
    put(image,opt+0x58,"<Q",0x100000)
    put(image,opt+0x60,"<Q",0x1000)
    put(image,opt+0x6C,"<I",16)
    put(image,opt+0x70+5*8,"<II",reloc_rva,8)

    sec=opt+0xF0
    image[sec:sec+8]=b".text\0\0\0"
    put(image,sec+0x08,"<I",len(code))
    put(image,sec+0x0C,"<I",text_rva)
    put(image,sec+0x10,"<I",raw_size)
    put(image,sec+0x14,"<I",text_raw)
    put(image,sec+0x24,"<I",0x60000020)

    reloc=sec+40
    image[reloc:reloc+8]=b".reloc\0\0"
    put(image,reloc+0x08,"<I",8)
    put(image,reloc+0x0C,"<I",reloc_rva)
    put(image,reloc+0x10,"<I",0x200)
    put(image,reloc+0x14,"<I",reloc_raw)
    put(image,reloc+0x24,"<I",0x42000040)

    image[text_raw:text_raw+len(code)]=code
    put(image,reloc_raw,"<II",text_rva,8)
    return bytes(image),labels

def main():
    if len(sys.argv)!=2:
        raise SystemExit("usage: build_uefi_interactive_access.py OUTPUT")
    image,labels=build_image()
    out=Path(sys.argv[1])
    out.parent.mkdir(parents=True,exist_ok=True)
    out.write_bytes(image)
    print("OS_UEFI_INTERACTIVE_ACCESS_BUILD=PASS")
    print("bytes="+str(len(image)))
    print("text-keybuf-offset=0x%x"%(0x200+labels["keybuf"]))
    print("sha256="+hashlib.sha256(image).hexdigest())

if __name__=="__main__":
    main()
