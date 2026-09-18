#!/usr/bin/env python3
from __future__ import annotations
import hashlib, struct, sys
from pathlib import Path

TEXT_RVA=0x1000
DATA_RVA=0x5000

MARKS={
 'start': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATE=START\r\nDOMAIN=PRE_OS_UEFI\r\nEND\r\n',
 'database': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nHII_DATABASE_PROTOCOL=PASS\r\nEND\r\n',
 'string_protocol': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nHII_STRING_PROTOCOL=PASS\r\nEND\r\n',
 'forms_handle': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nHII_FORMS_HANDLE=PASS\r\nEND\r\n',
 'first_handle': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nHII_FIRST_HANDLE_NONZERO=PASS\r\nEND\r\n',
 'static_empty': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_LIST_BUFFER_NOT_WRITTEN\r\nEND\r\n',
 'handle_export': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nHII_HANDLE_EXPORT=PASS\r\nEND\r\n',
 'forms_package_seen': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nHII_FORMS_PACKAGE_IN_SELECTED_HANDLE=PASS\r\nEND\r\n',
 'ifr': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nIFR_TITLE_STRING_ID=PASS\r\nEND\r\n',
 'language': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nHII_LANGUAGE=PASS\r\nEND\r\n',
 'prefix': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nTITLE_TEXT=',
 'suffix': b'\r\nHII_TITLE_STRING=PASS\r\nSTATUS=PASS\r\nEND\r\n',
 'no_protocol': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_PROTOCOL_NOT_FOUND\r\nEND\r\n',
 'list_size_fail': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_LIST_SIZE_QUERY_FAILED\r\nEND\r\n',
 'list_fetch_fail': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_LIST_FETCH_FAILED\r\nEND\r\n',
 'list_fetch_invalid': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_LIST_FETCH_EFI_INVALID_PARAMETER\r\nEND\r\n',
 'list_fetch_not_found': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_LIST_FETCH_EFI_NOT_FOUND\r\nEND\r\n',
 'list_fetch_bts_twice': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_LIST_FETCH_BUFFER_TOO_SMALL_TWICE\r\nEND\r\n',
 'list_static_small': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_LIST_STATIC_BUFFER_TOO_SMALL\r\nEND\r\n',
 'no_handle': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_FORMS_HANDLE_NOT_FOUND\r\nEND\r\n',
 'alloc': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=POOL_ALLOC_FAILED\r\nEND\r\n',
 'export': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_HANDLE_EXPORT_FAILED\r\nEND\r\n',
 'ifr_fail': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=IFR_TITLE_TOKEN_NOT_FOUND\r\nEND\r\n',
 'lang_fail': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_LANGUAGE_NOT_FOUND\r\nEND\r\n',
 'string_fail': b'QEVARYNOX-UEFI-HII-TITLE-V1\r\nSTATUS=BLOCKED\r\nREASON=HII_TITLE_STRING_NOT_RESOLVED\r\nEND\r\n',
}

class Code:
 def __init__(self):
  self.data=bytearray(); self.labels={}; self.fix=[]
 def pos(self): return len(self.data)
 def emit(self,b): self.data += bytes(b)
 def label(self,n): self.labels[n]=self.pos()
 def rel32(self,op,label):
  self.emit(op); off=self.pos(); self.emit(b'\0'*4); self.fix.append((off,self.pos(),label,4))
 def rel8(self,op,label):
  self.emit(bytes((op,0))); self.fix.append((self.pos()-1,self.pos(),label,1))
 def data_disp(self,opcode,off):
  self.emit(opcode); after=TEXT_RVA+self.pos()+4
  self.emit(struct.pack('<i',DATA_RVA+off-after))
 def lea_rax_data(self,off): self.data_disp(b'\x48\x8d\x05',off)
 def lea_rcx_data(self,off): self.data_disp(b'\x48\x8d\x0d',off)
 def lea_rdx_data(self,off): self.data_disp(b'\x48\x8d\x15',off)
 def lea_r8_data(self,off): self.data_disp(b'\x4c\x8d\x05',off)
 def lea_r9_data(self,off): self.data_disp(b'\x4c\x8d\x0d',off)
 def patch(self):
  for off,after,label,width in self.fix:
   disp=self.labels[label]-after
   if width==4: struct.pack_into('<i',self.data,off,disp)
   else:
    if not -128 <= disp <= 127: raise SystemExit(f'short jump overflow {label}: {disp}')
    self.data[off]=disp & 0xff

def put(b,o,f,*v): struct.pack_into(f,b,o,*v)

def build():
 data=bytearray(0x1100)
 L={
  'db_guid':0,'str_guid':16,'dbptr':32,'strptr':40,
  'handles_size':48,'handles_ptr':56,'pkg_size':64,'pkg_ptr':72,
  'langs_size':80,'langs_ptr':88,'string_size':96,'string_ptr':104,
  'token':112,'temp_handle':120,'handle_cursor':128,'handles_remaining':136,
  'handles_static':0x100,
 }
 struct.pack_into('<IHH8B',data,L['db_guid'],
  0xef9fc172,0xa1b2,0x4693,0xb3,0x27,0x6d,0x32,0xfc,0x41,0x60,0x42)
 struct.pack_into('<IHH8B',data,L['str_guid'],
  0x0fd96974,0x23aa,0x4cdc,0xb9,0xcb,0x98,0xd1,0x77,0x50,0x32,0x2a)
 for n,m in MARKS.items():
  L[n]=len(data); data+=m

 c=Code()
 c.emit(b'\x53\x55\x56\x57\x41\x54\x41\x55\x41\x56\x41\x57')
 c.emit(b'\x4c\x8b\x7a\x60')  # r15=BootServices
 c.emit(b'\x48\x83\xec\x68\xfc')

 for p,v in ((0x3f9,0),(0x3fb,0x80),(0x3f8,3),(0x3f9,0),(0x3fb,3),(0x3fa,0xc7),(0x3fc,0x0b)):
  c.emit(b'\x66\xba'+struct.pack('<H',p)+b'\xb0'+bytes((v,))+b'\xee')

 def serial(n):
  c.lea_rdx_data(L[n]); c.emit(b'\xb9'+struct.pack('<I',len(MARKS[n])))
  c.rel32(b'\xe8','serial_emit')
 def zero_qword(off):
  c.lea_rdx_data(off); c.emit(b'\x48\xc7\x02\x00\x00\x00\x00')
 def alloc(size_reg_is_rbx,ptr_off):
  c.emit(b'\xb9\x04\x00\x00\x00') # EfiBootServicesData
  if size_reg_is_rbx: c.emit(b'\x48\x89\xda')
  c.lea_r8_data(ptr_off)
  c.emit(b'\x41\xff\x57\x40')
  c.emit(b'\x48\x85\xc0'); c.rel32(b'\x0f\x85','fail_alloc')

 serial('start')

 # Locate HII Database protocol -> r12.
 c.lea_rcx_data(L['db_guid']); c.emit(b'\x31\xd2'); c.lea_r8_data(L['dbptr'])
 c.emit(b'\x41\xff\x97\x40\x01\x00\x00')
 c.emit(b'\x48\x85\xc0'); c.rel32(b'\x0f\x85','fail_protocol')
 c.lea_rdx_data(L['dbptr']); c.emit(b'\x4c\x8b\x22')
 c.emit(b'\x4d\x85\xe4'); c.rel32(b'\x0f\x84','fail_protocol')
 serial('database')

 # Locate HII String protocol -> r13.
 c.lea_rcx_data(L['str_guid']); c.emit(b'\x31\xd2'); c.lea_r8_data(L['strptr'])
 c.emit(b'\x41\xff\x97\x40\x01\x00\x00')
 c.emit(b'\x48\x85\xc0'); c.rel32(b'\x0f\x85','fail_protocol')
 c.lea_rdx_data(L['strptr']); c.emit(b'\x4c\x8b\x2a')
 c.emit(b'\x4d\x85\xed'); c.rel32(b'\x0f\x84','fail_protocol')
 serial('string_protocol')

 # Take one atomic snapshot of every active HII handle into bridge-owned
 # storage.  This avoids the observed OVMF race where a sizing query succeeds
 # but a second ListPackageLists call after AllocatePool returns EFI_NOT_FOUND.
 c.lea_rdx_data(L['handles_size'])
 c.emit(b'\x48\xc7\x02'+struct.pack('<I',0x1000))  # 512 EFI_HII_HANDLE slots
 c.emit(b'\x4c\x89\xe1\x31\xd2\x45\x31\xc0')  # This, ALL, Guid=NULL
 c.lea_r9_data(L['handles_size'])
 c.lea_rax_data(L['handles_static']); c.emit(b'\x48\x89\x44\x24\x20')
 c.emit(b'\x41\xff\x54\x24\x18')
 c.emit(b'\x48\x85\xc0'); c.rel32(b'\x0f\x84','list_fetch_ready')
 c.emit(b'\x83\xf8\x02'); c.rel32(b'\x0f\x84','fail_list_fetch_invalid')
 c.emit(b'\x83\xf8\x0e'); c.rel32(b'\x0f\x84','fail_list_fetch_not_found')
 c.emit(b'\x83\xf8\x05'); c.rel32(b'\x0f\x84','fail_list_static_small')
 c.rel32(b'\xe9','fail_list_fetch')
 c.label('list_fetch_ready')
 c.lea_rdx_data(L['handles_static']); c.emit(b'\x48\x83\x3a\x00')
 c.rel32(b'\x0f\x84','fail_static_empty')
 serial('first_handle')

 # Persist cursor and actual byte count; protocol calls may clobber volatile regs.
 c.lea_rax_data(L['handles_static'])
 c.lea_rdx_data(L['handle_cursor']); c.emit(b'\x48\x89\x02')
 c.lea_rdx_data(L['handles_size']); c.emit(b'\x48\x8b\x02')
 c.lea_rdx_data(L['handles_remaining']); c.emit(b'\x48\x89\x02')

 c.label('handle_loop')
 c.lea_rdx_data(L['handles_remaining']); c.emit(b'\x48\x8b\x2a')
 c.emit(b'\x48\x83\xfd\x08'); c.rel32(b'\x0f\x82','fail_no_handle')
 c.lea_rdx_data(L['handle_cursor']); c.emit(b'\x48\x8b\x32')
 c.emit(b'\x4c\x8b\x36')                    # r14 = current HII handle
 c.emit(b'\x48\x83\xc6\x08\x48\x83\xed\x08')
 c.lea_rdx_data(L['handle_cursor']); c.emit(b'\x48\x89\x32')
 c.lea_rdx_data(L['handles_remaining']); c.emit(b'\x48\x89\x2a')
 c.emit(b'\x4d\x85\xf6'); c.rel32(b'\x0f\x84','handle_loop')

 # Export this handle's package list, first obtaining the required byte count.
 zero_qword(L['pkg_size'])
 c.emit(b'\x4c\x89\xe1\x4c\x89\xf2')
 c.lea_r8_data(L['pkg_size']); c.emit(b'\x45\x31\xc9')
 c.emit(b'\x41\xff\x54\x24\x20')
 c.lea_rdx_data(L['pkg_size']); c.emit(b'\x48\x8b\x1a')
 c.emit(b'\x48\x83\xfb\x18'); c.rel32(b'\x0f\x82','handle_loop')
 alloc(True,L['pkg_ptr'])

 c.emit(b'\x4c\x89\xe1\x4c\x89\xf2')
 c.lea_r8_data(L['pkg_size'])
 c.lea_rdx_data(L['pkg_ptr']); c.emit(b'\x4c\x8b\x0a')
 c.emit(b'\x41\xff\x54\x24\x20')
 c.emit(b'\x48\x85\xc0'); c.rel32(b'\x0f\x85','handle_loop')
 serial('handle_export')

 # Verify that this exact HII handle owns a Forms package.
 c.lea_rdx_data(L['pkg_ptr']); c.emit(b'\x48\x8b\x32') # rsi=list
 c.emit(b'\x8b\x5e\x10\x83\xfb\x18'); c.rel32(b'\x0f\x82','fail_ifr')
 c.emit(b'\x48\x8d\x7e\x14\x83\xeb\x14') # rdi=package, ebx=remaining
 c.label('pkg_loop')
 c.emit(b'\x83\xfb\x04'); c.rel32(b'\x0f\x82','fail_ifr')
 c.emit(b'\x8b\x07\x89\xc2\x81\xe2\xff\xff\xff\x00')
 c.emit(b'\x89\xc5\xc1\xed\x18')
 c.emit(b'\x83\xfa\x04'); c.rel32(b'\x0f\x82','fail_ifr')
 c.emit(b'\x39\xda'); c.rel32(b'\x0f\x87','fail_ifr')
 c.emit(b'\x83\xfd\x02'); c.rel32(b'\x0f\x84','forms_pkg')
 c.emit(b'\x81\xfd\xdf\x00\x00\x00'); c.rel32(b'\x0f\x84','handle_loop')
 c.emit(b'\x48\x01\xd7\x29\xd3'); c.rel32(b'\xe9','pkg_loop')

 c.label('forms_pkg')
 serial('forms_package_seen')
 # r9=first IFR opcode, r10d=bytes available in verified Forms package.
 c.emit(b'\x4c\x8d\x4f\x04\x41\x89\xd2\x41\x83\xea\x04')
 serial('forms_handle')
 c.label('ifr_loop')
 c.emit(b'\x41\x83\xfa\x02'); c.rel32(b'\x0f\x82','fail_ifr')
 c.emit(b'\x41\x0f\xb6\x01')       # eax=OpCode
 c.emit(b'\x41\x0f\xb6\x49\x01\x83\xe1\x7f') # ecx=Length
 c.emit(b'\x83\xf9\x02'); c.rel32(b'\x0f\x82','fail_ifr')
 c.emit(b'\x44\x39\xd1'); c.rel32(b'\x0f\x87','fail_ifr')
 c.emit(b'\x3c\x0e'); c.rel32(b'\x0f\x84','ifr_formset')
 c.emit(b'\x3c\x01'); c.rel32(b'\x0f\x84','ifr_form')
 c.label('ifr_next')
 c.emit(b'\x49\x01\xc9\x41\x29\xca')
 c.rel32(b'\xe9','ifr_loop')

 c.label('ifr_formset')
 c.emit(b'\x83\xf9\x17'); c.rel32(b'\x0f\x82','ifr_next')
 c.emit(b'\x41\x0f\xb7\x41\x12'); c.rel32(b'\xe9','token_candidate')
 c.label('ifr_form')
 c.emit(b'\x83\xf9\x06'); c.rel32(b'\x0f\x82','ifr_next')
 c.emit(b'\x41\x0f\xb7\x41\x04')
 c.label('token_candidate')
 c.emit(b'\x66\x85\xc0'); c.rel32(b'\x0f\x84','ifr_next')
 c.lea_rdx_data(L['token']); c.emit(b'\x66\x89\x02')
 serial('ifr')

 # GetLanguages selected handle: size query.
 zero_qword(L['langs_size'])
 c.emit(b'\x4c\x89\xe9\x4c\x89\xf2\x45\x31\xc0')
 c.lea_r9_data(L['langs_size'])
 c.emit(b'\x41\xff\x55\x18')
 c.lea_rdx_data(L['langs_size']); c.emit(b'\x48\x8b\x1a')
 c.emit(b'\x48\x83\xfb\x02'); c.rel32(b'\x0f\x82','fail_language')
 alloc(True,L['langs_ptr'])

 # GetLanguages into buffer.
 c.emit(b'\x4c\x89\xe9\x4c\x89\xf2')
 c.lea_rdx_data(L['langs_ptr']); c.emit(b'\x4c\x8b\x02') # r8=buffer
 c.lea_r9_data(L['langs_size'])
 c.emit(b'\x41\xff\x55\x18')
 c.emit(b'\x48\x85\xc0'); c.rel32(b'\x0f\x85','fail_language')

 # Keep first RFC language tag: replace first ';' by NUL.
 c.lea_rdx_data(L['langs_ptr']); c.emit(b'\x48\x8b\x32')
 c.emit(b'\x80\x3e\x00'); c.rel32(b'\x0f\x84','fail_language')
 c.label('lang_scan')
 c.emit(b'\x8a\x06\x84\xc0'); c.rel32(b'\x0f\x84','lang_ready')
 c.emit(b'\x3c\x3b'); c.rel32(b'\x0f\x84','lang_cut')
 c.emit(b'\x48\xff\xc6'); c.rel32(b'\xe9','lang_scan')
 c.label('lang_cut'); c.emit(b'\xc6\x06\x00')
 c.label('lang_ready')
 serial('language')

 # GetString token: query UTF-16 byte size.
 zero_qword(L['string_size'])
 c.emit(b'\x4c\x89\xe9')
 c.lea_rdx_data(L['langs_ptr']); c.emit(b'\x48\x8b\x12')
 c.emit(b'\x4d\x89\xf0')
 c.lea_r9_data(L['token']); c.emit(b'\x45\x0f\xb7\x09')
 c.emit(b'\x48\xc7\x44\x24\x20\x00\x00\x00\x00')
 c.lea_rax_data(L['string_size']); c.emit(b'\x48\x89\x44\x24\x28')
 c.emit(b'\x48\xc7\x44\x24\x30\x00\x00\x00\x00')
 c.emit(b'\x41\xff\x55\x08')
 c.lea_rdx_data(L['string_size']); c.emit(b'\x48\x8b\x1a')
 c.emit(b'\x48\x83\xfb\x04'); c.rel32(b'\x0f\x82','fail_string')
 alloc(True,L['string_ptr'])

 # GetString into UTF-16 buffer.
 c.emit(b'\x4c\x89\xe9')
 c.lea_rdx_data(L['langs_ptr']); c.emit(b'\x48\x8b\x12')
 c.emit(b'\x4d\x89\xf0')
 c.lea_r9_data(L['token']); c.emit(b'\x45\x0f\xb7\x09')
 c.lea_rax_data(L['string_ptr']); c.emit(b'\x48\x8b\x00\x48\x89\x44\x24\x20')
 c.lea_rax_data(L['string_size']); c.emit(b'\x48\x89\x44\x24\x28')
 c.emit(b'\x48\xc7\x44\x24\x30\x00\x00\x00\x00')
 c.emit(b'\x41\xff\x55\x08')
 c.emit(b'\x48\x85\xc0'); c.rel32(b'\x0f\x85','fail_string')
 c.lea_rdx_data(L['string_ptr']); c.emit(b'\x48\x8b\x32')
 c.emit(b'\x66\x83\x3e\x00'); c.rel32(b'\x0f\x84','fail_string')

 # Emit a machine-readable ASCII projection of the real UTF-16 title.
 serial('prefix')
 c.rel32(b'\xe8','serial_utf16')
 serial('suffix')
 c.emit(b'\x31\xc0'); c.rel32(b'\xe9','return')

 c.label('fail_protocol'); serial('no_protocol'); c.rel32(b'\xe9','return_fail')
 c.label('fail_list_size'); serial('list_size_fail'); c.rel32(b'\xe9','return_fail')
 c.label('fail_list_fetch'); serial('list_fetch_fail'); c.rel32(b'\xe9','return_fail')
 c.label('fail_list_fetch_invalid'); serial('list_fetch_invalid'); c.rel32(b'\xe9','return_fail')
 c.label('fail_list_fetch_not_found'); serial('list_fetch_not_found'); c.rel32(b'\xe9','return_fail')
 c.label('fail_list_fetch_bts_twice'); serial('list_fetch_bts_twice'); c.rel32(b'\xe9','return_fail')
 c.label('fail_list_static_small'); serial('list_static_small'); c.rel32(b'\xe9','return_fail')
 c.label('fail_no_handle'); serial('no_handle'); c.rel32(b'\xe9','return_fail')
 c.label('fail_static_empty'); serial('static_empty'); c.rel32(b'\xe9','return_fail')
 c.label('fail_alloc'); serial('alloc'); c.rel32(b'\xe9','return_fail')
 c.label('fail_export'); serial('export'); c.rel32(b'\xe9','return_fail')
 c.label('fail_ifr'); serial('ifr_fail'); c.rel32(b'\xe9','return_fail')
 c.label('fail_language'); serial('lang_fail'); c.rel32(b'\xe9','return_fail')
 c.label('fail_string'); serial('string_fail')
 c.label('return_fail'); c.emit(b'\xb8\x01\x00\x00\x00')
 c.label('return')
 c.emit(b'\x48\x83\xc4\x68\x41\x5f\x41\x5e\x41\x5d\x41\x5c\x5f\x5e\x5d\x5b\xc3')

 # Input rsi=UTF-16 string. Emit low ASCII byte, '?' for non-ASCII codepoints.
 c.label('serial_utf16')
 c.label('utf16_loop')
 c.emit(b'\x66\x8b\x06\x66\x85\xc0'); c.rel32(b'\x0f\x84','utf16_done')
 c.emit(b'\x66\x3d\x7f\x00'); c.rel32(b'\x0f\x87','utf16_question')
 c.emit(b'\x3c\x20'); c.rel32(b'\x0f\x83','utf16_emit')
 c.label('utf16_question'); c.emit(b'\xb0\x3f')
 c.label('utf16_emit')
 c.emit(b'\x88\xc3\x66\xba\xfd\x03')
 c.label('utf16_wait'); c.emit(b'\xec\xa8\x20'); c.rel8(0x74,'utf16_wait')
 c.emit(b'\x66\xba\xf8\x03\x88\xd8\xee\x48\x83\xc6\x02')
 c.rel32(b'\xe9','utf16_loop')
 c.label('utf16_done'); c.emit(b'\xc3')

 c.label('serial_emit')
 c.emit(b'\x49\x89\xd0\x66\xba\xfd\x03')
 c.label('serial_wait'); c.emit(b'\xec\xa8\x20'); c.rel8(0x74,'serial_wait')
 c.emit(b'\x66\xba\xf8\x03\x41\x8a\x00\xee\x49\xff\xc0\x66\xba\xfd\x03\xff\xc9')
 c.rel8(0x75,'serial_wait'); c.emit(b'\xc3')

 c.patch(); code=bytes(c.data)
 if len(code)>0x3800: raise SystemExit(f'HII title code too large: {len(code)}')

 text_raw=0x200; text_raw_size=(len(code)+0x1ff)&~0x1ff
 data_raw=text_raw+text_raw_size; data_raw_size=(len(data)+0x1ff)&~0x1ff
 reloc_rva=(DATA_RVA+len(data)+0xfff)&~0xfff; reloc_raw=data_raw+data_raw_size
 image=bytearray(reloc_raw+0x200); image_size=reloc_rva+0x1000
 put(image,0,'<H',0x5a4d); put(image,0x3c,'<I',0x80)
 pe=0x80; image[pe:pe+4]=b'PE\0\0'; coff=pe+4
 put(image,coff,'<HHIIIHH',0x8664,3,0,0,0,0xf0,0x22); opt=coff+20
 put(image,opt,'<H',0x20b); put(image,opt+4,'<I',text_raw_size)
 put(image,opt+8,'<I',data_raw_size+0x200); put(image,opt+0x10,'<I',TEXT_RVA)
 put(image,opt+0x14,'<I',TEXT_RVA); put(image,opt+0x18,'<Q',0x400000)
 put(image,opt+0x20,'<I',0x1000); put(image,opt+0x24,'<I',0x200)
 put(image,opt+0x38,'<I',image_size); put(image,opt+0x3c,'<I',0x200)
 put(image,opt+0x44,'<H',10); put(image,opt+0x48,'<Q',0x100000)
 put(image,opt+0x50,'<Q',0x1000); put(image,opt+0x58,'<Q',0x100000)
 put(image,opt+0x60,'<Q',0x1000); put(image,opt+0x6c,'<I',16)
 put(image,opt+0x70+5*8,'<II',reloc_rva,8)
 sec=opt+0xf0; image[sec:sec+8]=b'.text\0\0\0'
 put(image,sec+8,'<I',len(code)); put(image,sec+0xc,'<I',TEXT_RVA)
 put(image,sec+0x10,'<I',text_raw_size); put(image,sec+0x14,'<I',text_raw)
 put(image,sec+0x24,'<I',0x60000020)
 ds=sec+40; image[ds:ds+8]=b'.data\0\0\0'
 put(image,ds+8,'<I',len(data)); put(image,ds+0xc,'<I',DATA_RVA)
 put(image,ds+0x10,'<I',data_raw_size); put(image,ds+0x14,'<I',data_raw)
 put(image,ds+0x24,'<I',0xc0000040)
 rs=sec+80; image[rs:rs+8]=b'.reloc\0\0'
 put(image,rs+8,'<I',8); put(image,rs+0xc,'<I',reloc_rva)
 put(image,rs+0x10,'<I',0x200); put(image,rs+0x14,'<I',reloc_raw)
 put(image,rs+0x24,'<I',0x42000040)
 image[text_raw:text_raw+len(code)]=code; image[data_raw:data_raw+len(data)]=data
 put(image,reloc_raw,'<II',TEXT_RVA,8)
 return bytes(image)

def validate(image):
 assert image[:2]==b'MZ'
 for token in (
  b'HII_DATABASE_PROTOCOL=PASS',
  b'HII_STRING_PROTOCOL=PASS',
  b'HII_FORMS_HANDLE=PASS',
  b'IFR_TITLE_STRING_ID=PASS',
  b'TITLE_TEXT=',
  b'HII_TITLE_STRING=PASS',
 ):
  assert token in image,token

def main():
 if len(sys.argv)!=2: raise SystemExit('usage: build_uefi_hii_title.py OUTPUT_EFI')
 image=build(); validate(image)
 p=Path(sys.argv[1]); p.parent.mkdir(parents=True,exist_ok=True); p.write_bytes(image)
 print('OS_UEFI_HII_TITLE_BUILD=PASS')
 print('bytes='+str(len(image)))
 print('sha256='+hashlib.sha256(image).hexdigest())

if __name__=='__main__':
 main()
