typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef unsigned long long usize;

#define PCI_CFG_ENABLE 0x80000000u
#define XHCI_PCI_CLASS 0x0c0330u
#define PCI_BAR0 0x10u
#define PCI_BAR1 0x14u

#define CAP_CAPLENGTH 0x00u
#define CAP_HCSPARAMS1 0x04u
#define CAP_HCSPARAMS2 0x08u
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
#define RT_IMAN0 0x20u
#define RT_ERSTSZ0 0x28u
#define RT_ERSTBA0 0x30u
#define RT_ERDP0 0x38u

#define USBCMD_RS (1u<<0)
#define USBCMD_HCRST (1u<<1)
#define USBSTS_HCH (1u<<0)
#define USBSTS_HSE (1u<<2)
#define USBSTS_CNR (1u<<11)
#define USBSTS_HCE (1u<<12)

#define PORTSC_CCS (1u<<0)
#define PORTSC_PED (1u<<1)
#define PORTSC_PR (1u<<4)
#define PORTSC_PP (1u<<9)
#define PORTSC_SPEED_SHIFT 10u
#define PORTSC_SPEED_MASK 0x0fu
#define PORTSC_CHANGE_BITS ((1u<<17)|(1u<<18)|(1u<<19)|(1u<<20)|(1u<<21)|(1u<<22)|(1u<<23))

#define TRB_CYCLE 1u
#define TRB_TYPE_SHIFT 10u
#define TRB_TYPE_ENABLE_SLOT 9u
#define TRB_TYPE_ADDRESS_DEVICE 11u
#define TRB_TYPE_COMMAND_COMPLETION 33u
#define TRB_TYPE_PORT_STATUS 34u
#define TRB_TYPE(x) (((x)>>TRB_TYPE_SHIFT)&0x3fu)

typedef struct __attribute__((packed,aligned(16))) {
    u64 parameter;
    u32 status;
    u32 control;
} trb;

typedef struct __attribute__((packed,aligned(64))) {
    u64 addr;
    u32 size;
    u32 reserved;
} erst_entry;

static u64 dcbaa[256] __attribute__((aligned(4096)));
static trb command_ring[256] __attribute__((aligned(4096)));
static trb event_ring[256] __attribute__((aligned(4096)));
static erst_entry erst[4] __attribute__((aligned(4096)));
static u8 input_context[33u*64u] __attribute__((aligned(4096)));
static u8 output_context[32u*64u] __attribute__((aligned(4096)));
static trb ep0_ring[256] __attribute__((aligned(4096)));

static volatile u8 *g_mmio;

static inline void outb(u16 p,u8 v){__asm__ volatile("outb %0,%1"::"a"(v),"d"(p));}
static inline u8 inb(u16 p){u8 v;__asm__ volatile("inb %1,%0":"=a"(v):"d"(p));return v;}
static inline void outl(u16 p,u32 v){__asm__ volatile("outl %0,%1"::"a"(v),"d"(p));}
static inline u32 inl(u16 p){u32 v;__asm__ volatile("inl %1,%0":"=a"(v):"d"(p));return v;}
static inline void fence(void){__asm__ volatile("mfence":::"memory");}
static void serial_init(void){outb(0x3f9,0);outb(0x3fb,0x80);outb(0x3f8,3);outb(0x3f9,0);outb(0x3fb,3);outb(0x3fa,0xc7);outb(0x3fc,0x0b);}
static void putc_s(char c){u32 t=1000000u;while(t--&&!(inb(0x3fd)&0x20u)){}outb(0x3f8,(u8)c);}
static void puts_s(const char*s){while(*s)putc_s(*s++);}
static void hex8(u8 v){static const char h[]="0123456789ABCDEF";putc_s(h[v>>4]);putc_s(h[v&15]);}
static void hex16(u16 v){hex8((u8)(v>>8));hex8((u8)v);}
static void hex32(u32 v){hex16((u16)(v>>16));hex16((u16)v);}
static void hex64(u64 v){hex32((u32)(v>>32));hex32((u32)v);}
static void line(const char*s){puts_s(s);puts_s("\r\n");}
static void kv32(const char*k,u32 v){puts_s(k);puts_s("=0x");hex32(v);puts_s("\r\n");}
static void kv64(const char*k,u64 v){puts_s(k);puts_s("=0x");hex64(v);puts_s("\r\n");}

static u32 pci_read32(u32 cfg){outl(0xcf8,cfg);return inl(0xcfc);}
static int find_xhci(u32*cfg,u32*vd){
    for(u32 bdf=0;bdf<0x10000u;++bdf){
        u32 c=PCI_CFG_ENABLE|(bdf<<8),v=pci_read32(c);
        if((v&0xffffu)==0xffffu)continue;
        if(((pci_read32(c|8u)>>8)&0xffffffu)==XHCI_PCI_CLASS){*cfg=c;*vd=v;return 1;}
    }
    return 0;
}
static int map_bar(u32 cfg,u64*bar){
    u32 lo=pci_read32(cfg|PCI_BAR0);if(lo&1u)return 0;
    u64 v=(u64)(lo&0xfffffff0u);
    if((lo&6u)==4u)v|=((u64)pci_read32(cfg|PCI_BAR1))<<32;
    if(!v)return 0;*bar=v;g_mmio=(volatile u8*)(usize)v;return 1;
}
static inline u8 mr8(u32 o){return *(volatile u8*)(g_mmio+o);}
static inline u32 mr32(u32 o){return *(volatile u32*)(g_mmio+o);}
static inline void mw32(u32 o,u32 v){*(volatile u32*)(g_mmio+o)=v;fence();}
static inline void mw64(u32 o,u64 v){mw32(o,(u32)v);mw32(o+4u,(u32)(v>>32));}
static void zero(void*p,usize n){volatile u8*d=(volatile u8*)p;while(n--)*d++=0;}
static int wait_mask(u32 off,u32 mask,u32 want,u32 loops){
    while(loops--){if((mr32(off)&mask)==want)return 1;for(volatile u32 d=0;d<2000u;++d)__asm__ volatile("":::"memory");}
    return 0;
}
static u8 find_connected_port(u32 op,u8 max_ports){
    for(u8 p=0;p<max_ports;++p){if(mr32(op+OP_PORTSC+0x10u*(u32)p)&PORTSC_CCS)return (u8)(p+1u);}
    return 0;
}
static int reset_usb2_port(u32 op,u8 port){
    u32 off=op+OP_PORTSC+0x10u*(u32)(port-1u);
    u32 ps=mr32(off);
    kv32("XHCI_PORT_PRE_RESET",ps);
    if(!(ps&PORTSC_CCS))return 0;
    if(ps&PORTSC_PED){line("XHCI_PORT_ALREADY_ENABLED=PASS");return 1;}
    u32 v=ps;
    v&=~(PORTSC_PED|PORTSC_CHANGE_BITS);
    v|=PORTSC_PR;
    mw32(off,v);
    if(!wait_mask(off,PORTSC_PR,0u,400000u))return 0;
    ps=mr32(off);kv32("XHCI_PORT_POST_RESET",ps);
    return ((ps&PORTSC_CCS)&&(ps&PORTSC_PED))?1:0;
}
static int wait_command_completion(u64 expected_ptr,u8*slot_out,u8*cc_out){
    for(u32 spin=0;spin<600000u;++spin){
        for(u32 i=0;i<64u;++i){
            u32 ctl=event_ring[i].control;
            if(!(ctl&TRB_CYCLE))continue;
            u32 type=TRB_TYPE(ctl);
            u8 cc=(u8)(event_ring[i].status>>24);
            if(type==TRB_TYPE_PORT_STATUS){
                puts_s("XHCI_EVENT_PORT_STATUS index=0x");hex32(i);
                puts_s(" cc=0x");hex8(cc);puts_s("\r\n");
            }
            if(type==TRB_TYPE_COMMAND_COMPLETION &&
               ((event_ring[i].parameter&~0xfull)==(expected_ptr&~0xfull))){
                *slot_out=(u8)(ctl>>24);*cc_out=cc;
                puts_s("XHCI_EVENT_COMMAND_COMPLETE index=0x");hex32(i);
                puts_s(" cc=0x");hex8(cc);puts_s(" slot=0x");hex8(*slot_out);
                puts_s(" ptr=0x");hex64(event_ring[i].parameter);puts_s("\r\n");
                return 1;
            }
        }
        for(volatile u32 d=0;d<300u;++d)__asm__ volatile("":::"memory");
    }
    return 0;
}
static void wait_microframes(u32 rt,u32 count){
    u32 start=mr32(rt)&0x3fffu;
    for(;;){
        u32 now=mr32(rt)&0x3fffu;
        u32 delta=(now-start)&0x3fffu;
        if(delta>=count)break;
    }
}
static void ctx32(u8*base,u32 off,u32 value){*(volatile u32*)(base+off)=value;}
static u32 ctxr32(const u8*base,u32 off){return *(const volatile u32*)(base+off);}
static void ctx64(u8*base,u32 off,u64 value){
    ctx32(base,off,(u32)value);
    ctx32(base,off+4u,(u32)(value>>32));
}

__attribute__((ms_abi)) u64 efi_main(void*image_handle,void*system_table){
    (void)image_handle;
    serial_init();
    line("QEVARYNOX-UEFI-XHCI-DIRECT-STAGE3-V1");
    line("MODE=DESTRUCTIVE_QEMU_XHCI_ADDRESS_DEVICE");
    line("STATE=START");

    u32 cfg=0,vd=0;u64 bar=0;
    if(!find_xhci(&cfg,&vd)||!map_bar(cfg,&bar)){line("STATUS=BLOCKED");line("REASON=XHCI_DISCOVERY");return 1;}
    puts_s("XHCI_PCI_VENDOR=0x");hex16((u16)vd);puts_s("\r\n");
    puts_s("XHCI_PCI_DEVICE=0x");hex16((u16)(vd>>16));puts_s("\r\n");
    kv64("XHCI_BAR0",bar);

    u8 caplen=mr8(CAP_CAPLENGTH);
    u32 hcs1=mr32(CAP_HCSPARAMS1);
    u32 hcs2=mr32(CAP_HCSPARAMS2);
    u32 hcc1=mr32(CAP_HCCPARAMS1);
    u32 dboff=mr32(CAP_DBOFF)&~3u;
    u32 rtsoff=mr32(CAP_RTSOFF)&~0x1fu;
    u32 op=(u32)caplen;
    u8 max_ports=(u8)(hcs1>>24);
    kv32("XHCI_CAP_DWORD0",mr32(0));
    kv32("XHCI_HCSPARAMS1",hcs1);
    kv32("XHCI_HCCPARAMS1",hcc1);
    if(!caplen||!max_ports||!dboff||!rtsoff){line("STATUS=BLOCKED");line("REASON=XHCI_CAPS");return 1;}
    line("XHCI_DISCOVERY=PASS");

    u8 original_port=find_connected_port(op,max_ports);
    if(!original_port){line("STATUS=BLOCKED");line("REASON=NO_CONNECTED_PORT_BEFORE_RESET");return 1;}
    puts_s("XHCI_CONNECTED_PORT_PRE_RESET=0x");hex8(original_port);puts_s("\r\n");

    u32 cmd=mr32(op+OP_USBCMD);
    mw32(op+OP_USBCMD,cmd&~USBCMD_RS);
    if(!wait_mask(op+OP_USBSTS,USBSTS_HCH,USBSTS_HCH,400000u)){
        kv32("XHCI_USBSTS_STOP_TIMEOUT",mr32(op+OP_USBSTS));line("STATUS=BLOCKED");line("REASON=HALT_TIMEOUT");return 1;
    }
    line("XHCI_HALT=PASS");

    mw32(op+OP_USBCMD,mr32(op+OP_USBCMD)|USBCMD_HCRST);
    if(!wait_mask(op+OP_USBCMD,USBCMD_HCRST,0u,800000u)){
        line("STATUS=BLOCKED");line("REASON=HCRST_TIMEOUT");return 1;
    }
    if(!wait_mask(op+OP_USBSTS,USBSTS_CNR,0u,800000u)){
        line("STATUS=BLOCKED");line("REASON=CNR_TIMEOUT");return 1;
    }
    if(mr32(op+OP_USBSTS)&(USBSTS_HSE|USBSTS_HCE)){
        kv32("XHCI_USBSTS_AFTER_RESET",mr32(op+OP_USBSTS));line("STATUS=BLOCKED");line("REASON=RESET_ERROR");return 1;
    }
    line("XHCI_HCRST=PASS");

    zero(dcbaa,sizeof(dcbaa));zero(command_ring,sizeof(command_ring));zero(event_ring,sizeof(event_ring));zero(erst,sizeof(erst));zero(input_context,sizeof(input_context));zero(output_context,sizeof(output_context));zero(ep0_ring,sizeof(ep0_ring));
    line("XHCI_DMA_MEMORY_ZEROED=PASS");
    kv64("XHCI_DCBAA_ADDR",(u64)(usize)dcbaa);
    kv64("XHCI_COMMAND_RING_ADDR",(u64)(usize)command_ring);
    kv64("XHCI_EVENT_RING_ADDR",(u64)(usize)event_ring);
    kv64("XHCI_ERST_ADDR",(u64)(usize)erst);

    mw32(op+OP_CONFIG,1u);
    mw64(op+OP_DCBAAP,(u64)(usize)dcbaa);
    mw64(op+OP_CRCR,((u64)(usize)command_ring&~0x3full)|1ull);
    line("XHCI_DCBAA_COMMAND_RING_PROGRAMMED=PASS");

    erst[0].addr=(u64)(usize)event_ring;
    erst[0].size=256u;
    erst[0].reserved=0u;
    fence();
    mw32(rtsoff+RT_IMAN0,0u);
    mw32(rtsoff+RT_ERSTSZ0,1u);
    mw64(rtsoff+RT_ERDP0,(u64)(usize)event_ring);
    mw64(rtsoff+RT_ERSTBA0,(u64)(usize)erst);
    line("XHCI_EVENT_RING_PROGRAMMED=PASS");

    mw32(op+OP_USBCMD,USBCMD_RS);
    if(!wait_mask(op+OP_USBSTS,USBSTS_HCH,0u,400000u)){
        kv32("XHCI_USBSTS_RUN_TIMEOUT",mr32(op+OP_USBSTS));line("STATUS=BLOCKED");line("REASON=RUN_TIMEOUT");return 1;
    }
    if(mr32(op+OP_USBSTS)&(USBSTS_HSE|USBSTS_HCE)){
        kv32("XHCI_USBSTS_RUN_ERROR",mr32(op+OP_USBSTS));line("STATUS=BLOCKED");line("REASON=RUN_ERROR");return 1;
    }
    line("XHCI_RUN_AFTER_REINIT=PASS");

    u8 port=find_connected_port(op,max_ports);
    if(!port){line("STATUS=BLOCKED");line("REASON=NO_CONNECTED_PORT_AFTER_RESET");return 1;}
    puts_s("XHCI_CONNECTED_PORT_POST_RESET=0x");hex8(port);puts_s("\r\n");
    if(!reset_usb2_port(op,port)){line("STATUS=BLOCKED");line("REASON=PORT_RESET_ENABLE");return 1;}
    line("XHCI_ROOT_PORT_READY=PASS");
    u32 ps=mr32(op+OP_PORTSC+0x10u*(u32)(port-1u));
    puts_s("XHCI_PORT_SPEED=0x");hex8((u8)((ps>>PORTSC_SPEED_SHIFT)&PORTSC_SPEED_MASK));puts_s("\r\n");

    command_ring[0].parameter=0;
    command_ring[0].status=0;
    command_ring[0].control=(TRB_TYPE_ENABLE_SLOT<<TRB_TYPE_SHIFT)|TRB_CYCLE;
    fence();
    kv32("XHCI_ENABLE_SLOT_TRB_CONTROL",command_ring[0].control);
    mw32(dboff,0u);
    line("XHCI_HOST_DOORBELL_RUNG=PASS");

    u8 slot=0,cc=0;
    if(!wait_command_completion((u64)(usize)&command_ring[0],&slot,&cc)){
        kv32("XHCI_USBSTS_COMMAND_TIMEOUT",mr32(op+OP_USBSTS));line("STATUS=BLOCKED");line("REASON=COMMAND_COMPLETION_TIMEOUT");return 1;
    }
    puts_s("XHCI_ENABLE_SLOT_COMPLETION_CODE=0x");hex8(cc);puts_s("\r\n");
    puts_s("XHCI_ENABLE_SLOT_ID=0x");hex8(slot);puts_s("\r\n");
    if(cc!=1u||slot==0u){line("STATUS=BLOCKED");line("REASON=ENABLE_SLOT_FAILED");return 1;}
    line("XHCI_ENABLE_SLOT=PASS");

    u8 speed=(u8)((ps>>PORTSC_SPEED_SHIFT)&PORTSC_SPEED_MASK);
    u32 ctx_size=(hcc1&(1u<<2))?64u:32u;
    puts_s("XHCI_CONTEXT_SIZE_BYTES=0x");hex32(ctx_size);puts_s("\r\n");
    if(ctx_size!=32u&&ctx_size!=64u){line("STATUS=BLOCKED");line("REASON=CONTEXT_SIZE_INVALID");return 1;}

    u8*ctrl=input_context;
    u8*slot_ctx=input_context+ctx_size;
    u8*ep0_ctx=input_context+2u*ctx_size;
    ctx32(ctrl,4u,0x00000003u);
    ctx32(slot_ctx,0u,((u32)speed<<20)|(1u<<27));
    ctx32(slot_ctx,4u,((u32)port<<16));

    u32 ep0_mps=(speed==4u)?512u:((speed==3u)?64u:8u);
    ctx32(ep0_ctx,0u,0u);
    ctx32(ep0_ctx,4u,(ep0_mps<<16)|(4u<<3)|(3u<<1));
    ctx64(ep0_ctx,8u,((u64)(usize)ep0_ring)|1ull);
    ctx32(ep0_ctx,16u,8u);
    fence();

    dcbaa[slot]=(u64)(usize)output_context;
    fence();
    kv64("XHCI_INPUT_CONTEXT_ADDR",(u64)(usize)input_context);
    kv64("XHCI_OUTPUT_CONTEXT_ADDR",(u64)(usize)output_context);
    kv64("XHCI_EP0_RING_ADDR",(u64)(usize)ep0_ring);
    kv32("XHCI_INPUT_ADD_FLAGS",ctxr32(ctrl,4u));
    kv32("XHCI_INPUT_SLOT_DWORD0",ctxr32(slot_ctx,0u));
    kv32("XHCI_INPUT_SLOT_DWORD1",ctxr32(slot_ctx,4u));
    kv32("XHCI_INPUT_EP0_INFO2",ctxr32(ep0_ctx,4u));
    kv32("XHCI_INPUT_EP0_TXINFO",ctxr32(ep0_ctx,16u));
    puts_s("XHCI_INITIAL_EP0_MPS=0x");hex32(ep0_mps);puts_s("\r\n");
    line("XHCI_INPUT_CONTEXT=PASS");

    wait_microframes(rtsoff,80u);
    line("XHCI_RESET_RECOVERY_10MS=PASS");

    command_ring[1].parameter=(u64)(usize)input_context;
    command_ring[1].status=0u;
    command_ring[1].control=((u32)slot<<24)|(TRB_TYPE_ADDRESS_DEVICE<<TRB_TYPE_SHIFT)|TRB_CYCLE;
    fence();
    kv32("XHCI_ADDRESS_DEVICE_TRB_CONTROL",command_ring[1].control);
    mw32(dboff,0u);
    line("XHCI_ADDRESS_DEVICE_DOORBELL=PASS");

    u8 addr_slot=0,addr_cc=0;
    if(!wait_command_completion((u64)(usize)&command_ring[1],&addr_slot,&addr_cc)){
        kv32("XHCI_USBSTS_ADDRESS_TIMEOUT",mr32(op+OP_USBSTS));
        line("STATUS=BLOCKED");line("REASON=ADDRESS_DEVICE_COMPLETION_TIMEOUT");return 1;
    }
    puts_s("XHCI_ADDRESS_DEVICE_COMPLETION_CODE=0x");hex8(addr_cc);puts_s("\r\n");
    puts_s("XHCI_ADDRESS_DEVICE_SLOT_ID=0x");hex8(addr_slot);puts_s("\r\n");
    if(addr_cc!=1u||addr_slot!=slot){
        line("STATUS=BLOCKED");line("REASON=ADDRESS_DEVICE_FAILED");return 1;
    }

    fence();
    u8*out_slot=output_context;
    u8*out_ep0=output_context+ctx_size;
    u32 out_state=ctxr32(out_slot,12u);
    u8 dev_addr=(u8)(out_state&0xffu);
    u8 slot_state=(u8)((out_state>>27)&0x1fu);
    u8 ep0_state=(u8)(ctxr32(out_ep0,0u)&0x7u);
    kv32("XHCI_OUTPUT_SLOT_DWORD0",ctxr32(out_slot,0u));
    kv32("XHCI_OUTPUT_SLOT_DWORD1",ctxr32(out_slot,4u));
    kv32("XHCI_OUTPUT_SLOT_STATE",out_state);
    puts_s("XHCI_DEVICE_ADDRESS=0x");hex8(dev_addr);puts_s("\r\n");
    puts_s("XHCI_SLOT_STATE=0x");hex8(slot_state);puts_s("\r\n");
    puts_s("XHCI_EP0_STATE=0x");hex8(ep0_state);puts_s("\r\n");
    if(dev_addr==0u||slot_state!=2u){
        line("STATUS=BLOCKED");line("REASON=OUTPUT_CONTEXT_NOT_ADDRESSED");return 1;
    }

    line("XHCI_ADDRESS_DEVICE=PASS");
    line("XHCI_OUTPUT_CONTEXT_ADDRESSED=PASS");
    line("XHCI_COMMAND_RING=PASS");
    line("XHCI_EVENT_RING=PASS");
    line("XHCI_DIRECT_TAKEOVER_STAGE3=PASS");
    line("XHCI_GET_DESCRIPTOR=NOT_ATTEMPTED");
    line("XHCI_ISOCHRONOUS_RING=NOT_ATTEMPTED");
    line("STATUS=PASS");
    return 0;
}
