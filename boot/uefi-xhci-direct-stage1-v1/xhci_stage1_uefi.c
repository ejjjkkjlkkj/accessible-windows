typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef unsigned long long usize;

#define XHCI_PCI_CLASS 0x0c0330u
#define PCI_CFG_ENABLE 0x80000000u
#define PCI_BAR0 0x10u
#define PCI_BAR1 0x14u

#define CAP_CAPLENGTH 0x00u
#define CAP_HCIVERSION 0x02u
#define CAP_HCSPARAMS1 0x04u
#define CAP_HCSPARAMS2 0x08u
#define CAP_HCSPARAMS3 0x0cu
#define CAP_HCCPARAMS1 0x10u
#define CAP_DBOFF 0x14u
#define CAP_RTSOFF 0x18u

#define OP_USBCMD 0x00u
#define OP_USBSTS 0x04u
#define OP_PAGESIZE 0x08u
#define OP_CRCR 0x18u
#define OP_DCBAAP 0x30u
#define OP_CONFIG 0x38u
#define OP_PORTSC 0x400u

#define USBSTS_HCH (1u << 0)
#define USBSTS_CNR (1u << 11)
#define PORTSC_CCS (1u << 0)
#define PORTSC_PED (1u << 1)
#define PORTSC_PR  (1u << 4)
#define PORTSC_PLS_SHIFT 5u
#define PORTSC_SPEED_SHIFT 10u
#define PORTSC_SPEED_MASK 0x0fu

static inline void outb(u16 port, u8 value) { __asm__ volatile("outb %0, %1" :: "a"(value), "d"(port)); }
static inline u8 inb(u16 port) { u8 v; __asm__ volatile("inb %1, %0" : "=a"(v) : "d"(port)); return v; }
static inline void outl(u16 port, u32 value) { __asm__ volatile("outl %0, %1" :: "a"(value), "d"(port)); }
static inline u32 inl(u16 port) { u32 v; __asm__ volatile("inl %1, %0" : "=a"(v) : "d"(port)); return v; }

static void serial_init(void) {
    outb(0x3f9,0); outb(0x3fb,0x80); outb(0x3f8,3); outb(0x3f9,0);
    outb(0x3fb,3); outb(0x3fa,0xc7); outb(0x3fc,0x0b);
}
static void serial_char(char c) {
    u32 t=1000000u; while (t-- && !(inb(0x3fd)&0x20u)) {} outb(0x3f8,(u8)c);
}
static void puts_s(const char *s) { while(*s) serial_char(*s++); }
static void hex8(u8 v) { static const char h[]="0123456789ABCDEF"; serial_char(h[v>>4]); serial_char(h[v&15]); }
static void hex16(u16 v) { hex8((u8)(v>>8)); hex8((u8)v); }
static void hex32(u32 v) { hex16((u16)(v>>16)); hex16((u16)v); }
static void hex64(u64 v) { hex32((u32)(v>>32)); hex32((u32)v); }
static void line(const char *s) { puts_s(s); puts_s("\r\n"); }
static void kv32(const char *k,u32 v){puts_s(k);puts_s("=0x");hex32(v);puts_s("\r\n");}
static void kv64(const char *k,u64 v){puts_s(k);puts_s("=0x");hex64(v);puts_s("\r\n");}

static u32 pci_read32(u32 cfg) { outl(0xcf8,cfg); return inl(0xcfc); }

static volatile u8 *g_mmio;
static inline u8 mr8(u32 o){return *(volatile u8 *)(g_mmio+o);}
static inline u16 mr16(u32 o){return *(volatile u16 *)(g_mmio+o);}
static inline u32 mr32(u32 o){return *(volatile u32 *)(g_mmio+o);}
static inline u64 mr64(u32 o){u32 lo=mr32(o),hi=mr32(o+4u);return ((u64)hi<<32)|lo;}

static int find_xhci(u32 *cfg_out,u32 *vd_out) {
    for (u32 bdf=0;bdf<0x10000u;++bdf) {
        u32 cfg=PCI_CFG_ENABLE|(bdf<<8);
        u32 vd=pci_read32(cfg);
        if ((vd&0xffffu)==0xffffu) continue;
        u32 classreg=pci_read32(cfg|0x08u);
        if (((classreg>>8)&0xffffffu)!=XHCI_PCI_CLASS) continue;
        *cfg_out=cfg; *vd_out=vd; return 1;
    }
    return 0;
}
static int map_bar(u32 cfg,u64 *bar_out) {
    u32 lo=pci_read32(cfg|PCI_BAR0);
    if (lo&1u) return 0;
    u64 bar=(u64)(lo&0xfffffff0u);
    if ((lo&0x6u)==0x4u) bar|=((u64)pci_read32(cfg|PCI_BAR1))<<32;
    if (!bar) return 0;
    *bar_out=bar; g_mmio=(volatile u8 *)(usize)bar; return 1;
}
static void scan_extended_caps(u32 hcc1,u8 max_ports) {
    u32 p=((hcc1>>16)&0xffffu)*4u;
    u32 guard=0;
    u32 proto_count=0;
    while (p && guard++<64u) {
        u32 dw0=mr32(p);
        u8 id=(u8)(dw0&0xffu);
        u8 next=(u8)((dw0>>8)&0xffu);
        if (id==2u) {
            u8 minor=(u8)((dw0>>16)&0xffu);
            u8 major=(u8)((dw0>>24)&0xffu);
            u32 dw2=mr32(p+8u);
            u8 port_off=(u8)(dw2&0xffu);
            u8 port_count=(u8)((dw2>>8)&0xffu);
            puts_s("XHCI_SUPPORTED_PROTOCOL major=0x");hex8(major);
            puts_s(" minor=0x");hex8(minor);
            puts_s(" port-off=0x");hex8(port_off);
            puts_s(" port-count=0x");hex8(port_count);puts_s("\r\n");
            if (major==2u) line("XHCI_SUPPORTED_PROTOCOL_USB2=PASS");
            if (major==3u) line("XHCI_SUPPORTED_PROTOCOL_USB3=PASS");
            if (port_off && port_count && (u32)port_off+(u32)port_count-1u<=(u32)max_ports)
                line("XHCI_SUPPORTED_PROTOCOL_PORT_RANGE=PASS");
            ++proto_count;
        }
        if (!next) break;
        p+=((u32)next)*4u;
    }
    if (proto_count) line("XHCI_EXTENDED_CAPABILITIES=PASS");
    else line("XHCI_EXTENDED_CAPABILITIES=NONE_OR_NOT_EXPOSED");
}
__attribute__((ms_abi)) u64 efi_main(void *image_handle,void *system_table) {
    (void)image_handle;(void)system_table;
    serial_init();
    line("QEVARYNOX-UEFI-XHCI-DIRECT-STAGE1-V1");
    line("STATE=START");
    line("MODE=NON_DESTRUCTIVE_DIRECT_MMIO");

    u32 cfg=0,vd=0;
    if(!find_xhci(&cfg,&vd)){line("STATUS=BLOCKED");line("REASON=XHCI_PCI_NOT_FOUND");return 1;}
    line("XHCI_PCI_CLASS_0C0330=PASS");
    puts_s("XHCI_PCI_VENDOR=0x");hex16((u16)(vd&0xffffu));puts_s("\r\n");
    puts_s("XHCI_PCI_DEVICE=0x");hex16((u16)(vd>>16));puts_s("\r\n");
    kv32("XHCI_PCI_CFG_BASE",cfg);
    u32 cmd=pci_read32(cfg|0x04u)&0xffffu;
    kv32("XHCI_PCI_COMMAND",cmd);
    if ((cmd&0x2u)!=0u) line("XHCI_PCI_MEMORY_ENABLE=PASS");

    u64 bar=0;
    if(!map_bar(cfg,&bar)){line("STATUS=BLOCKED");line("REASON=XHCI_BAR_INVALID");return 1;}
    kv64("XHCI_BAR0",bar);
    line("XHCI_MMIO_MAP=PASS");

    u8 caplen=mr8(CAP_CAPLENGTH);
    u16 hciver=mr16(CAP_HCIVERSION);
    u32 hcs1=mr32(CAP_HCSPARAMS1);
    u32 hcs2=mr32(CAP_HCSPARAMS2);
    u32 hcs3=mr32(CAP_HCSPARAMS3);
    u32 hcc1=mr32(CAP_HCCPARAMS1);
    u32 dboff=mr32(CAP_DBOFF)&~3u;
    u32 rtsoff=mr32(CAP_RTSOFF)&~0x1fu;
    if (caplen<0x20u || caplen>0x80u || !dboff || !rtsoff) {
        line("STATUS=BLOCKED");line("REASON=XHCI_CAPABILITY_LAYOUT_INVALID");return 1;
    }
    puts_s("XHCI_CAPLENGTH=0x");hex8(caplen);puts_s("\r\n");
    puts_s("XHCI_HCIVERSION=0x");hex16(hciver);puts_s("\r\n");
    kv32("XHCI_HCSPARAMS1",hcs1);kv32("XHCI_HCSPARAMS2",hcs2);kv32("XHCI_HCSPARAMS3",hcs3);
    kv32("XHCI_HCCPARAMS1",hcc1);kv32("XHCI_DBOFF",dboff);kv32("XHCI_RTSOFF",rtsoff);
    line("XHCI_CAPABILITY_REGISTERS=PASS");

    u8 max_slots=(u8)(hcs1&0xffu);
    u16 max_intrs=(u16)((hcs1>>8)&0x7ffu);
    u8 max_ports=(u8)(hcs1>>24);
    puts_s("XHCI_MAX_SLOTS=0x");hex8(max_slots);puts_s("\r\n");
    puts_s("XHCI_MAX_INTRS=0x");hex16(max_intrs);puts_s("\r\n");
    puts_s("XHCI_MAX_PORTS=0x");hex8(max_ports);puts_s("\r\n");
    if (!max_slots||!max_ports){line("STATUS=BLOCKED");line("REASON=XHCI_ZERO_CAPACITY");return 1;}
    if (hcc1&1u) line("XHCI_64BIT_ADDRESSING=PASS");
    if (hcc1&(1u<<2)) line("XHCI_CONTEXT_SIZE=64");
    else line("XHCI_CONTEXT_SIZE=32");

    u32 op=(u32)caplen;
    u32 usbcmd=mr32(op+OP_USBCMD);
    u32 usbsts=mr32(op+OP_USBSTS);
    u32 pagesize=mr32(op+OP_PAGESIZE);
    u64 crcr=mr64(op+OP_CRCR);
    u64 dcbaap=mr64(op+OP_DCBAAP);
    u32 config=mr32(op+OP_CONFIG);
    kv32("XHCI_USBCMD",usbcmd);kv32("XHCI_USBSTS",usbsts);kv32("XHCI_PAGESIZE",pagesize);
    kv64("XHCI_CRCR",crcr);kv64("XHCI_DCBAAP",dcbaap);kv32("XHCI_CONFIG",config);
    if (!(usbsts&USBSTS_CNR)) line("XHCI_CONTROLLER_READY=PASS");
    if (usbsts&USBSTS_HCH) line("XHCI_CONTROLLER_STATE=HALTED");
    else line("XHCI_CONTROLLER_STATE=RUNNING");

    u32 mf1=mr32(rtsoff)&0x3fffu;
    for (volatile u32 spin=0;spin<1000000u;++spin) __asm__ volatile("" ::: "memory");
    u32 mf2=mr32(rtsoff)&0x3fffu;
    kv32("XHCI_MFINDEX_1",mf1);kv32("XHCI_MFINDEX_2",mf2);
    if (!(usbsts&USBSTS_HCH) && mf1!=mf2) line("XHCI_RUNTIME_CLOCK=PASS");
    else line("XHCI_RUNTIME_CLOCK=STATIC");

    scan_extended_caps(hcc1,max_ports);

    u32 connected=0,enabled=0;
    for(u8 p=0;p<max_ports;++p){
        u32 ps=mr32(op+OP_PORTSC+0x10u*(u32)p);
        if(ps&PORTSC_CCS){
            ++connected;
            puts_s("XHCI_PORT_CONNECTED index=0x");hex8((u8)(p+1u));
            puts_s(" portsc=0x");hex32(ps);
            puts_s(" speed=0x");hex8((u8)((ps>>PORTSC_SPEED_SHIFT)&PORTSC_SPEED_MASK));
            puts_s(" pls=0x");hex8((u8)((ps>>PORTSC_PLS_SHIFT)&0x0fu));
            puts_s("\r\n");
            if(ps&PORTSC_PED) ++enabled;
            if(ps&PORTSC_PR) line("XHCI_PORT_RESET_ACTIVE=OBSERVED");
        }
    }
    puts_s("XHCI_CONNECTED_PORTS=0x");hex32(connected);puts_s("\r\n");
    puts_s("XHCI_ENABLED_PORTS=0x");hex32(enabled);puts_s("\r\n");
    if(connected) line("XHCI_PORT_SCAN=PASS");
    else {line("STATUS=BLOCKED");line("REASON=XHCI_NO_CONNECTED_PORT");return 1;}

    line("XHCI_DIRECT_MMIO_OWNERSHIP_BASELINE=PASS");
    line("XHCI_DESTRUCTIVE_RESET=NOT_ATTEMPTED");
    line("XHCI_COMMAND_RING=NOT_ATTEMPTED");
    line("XHCI_ISOCHRONOUS_RING=NOT_ATTEMPTED");
    line("STATUS=PASS");
    return 0;
}
