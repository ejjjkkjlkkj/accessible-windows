typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef unsigned long long usize;

typedef u64 (*stall_fn)(usize microseconds);
typedef u64 (*allocate_pages_fn)(u32 type, u32 memory_type, usize pages, u64 *memory);
typedef u64 (*locate_protocol_fn)(const void *protocol, void *registration, void **interface_out);
typedef u64 (*hii_export_fn)(void *self, void *handle, usize *buffer_size, void *buffer);

extern const u8 qev_unit_bank[];
extern const u32 qev_unit_bank_len;
extern const u32 qev_unit_offsets[];
extern const u32 qev_unit_lengths[];
extern const u32 qev_unit_count;
extern const u8 qev_letter_count[];
extern const u8 qev_letter_units[];
extern const u32 qev_letter_stride;

#define MAX_NID 256
#define MAX_CONN 64
#define INVALID_NID 0xff
#define INVALID_RESP 0xffffffffu
#define MAX_TEXT 8
#define MAX_FORMS 16
#define MAX_STRINGS 32
#define HII_BUFFER_BYTES (1024u * 1024u)
#define DMA_PAGES 128u
#define PCM_OFF 0x1000u
#define MAX_BDL 96u

#define WIDGET_AUDIO_OUTPUT 0x0
#define WIDGET_AUDIO_INPUT  0x1
#define WIDGET_MIXER        0x2
#define WIDGET_SELECTOR     0x3
#define WIDGET_PIN          0x4
#define WIDGET_POWER        0x5
#define WIDGET_VOLUME       0x6
#define WIDGET_VENDOR       0xf

struct guid {
    u32 d1; u16 d2; u16 d3; u8 d4[8];
};
struct pkg_ref { const u8 *p; u32 len; };

static const struct guid g_hii_db_guid = {
    0xef9fc172u, 0xa1b2u, 0x4693u, {0xb3,0x27,0x6d,0x32,0xfc,0x41,0x60,0x42}
};

static u8 g_type[MAX_NID];
static u8 g_conn_count[MAX_NID];
static u8 g_conn[MAX_NID][MAX_CONN];
static u8 g_pin_output[MAX_NID];
static u8 g_seen[MAX_NID];
static u8 g_parent[MAX_NID];
static u8 g_depth[MAX_NID];
static u8 g_queue[MAX_NID];
static u8 g_route_index[MAX_NID];
static u8 g_hii_buffer[HII_BUFFER_BYTES];
static char g_text[MAX_TEXT + 1];
static u8 g_text_len;

static volatile u8 *g_hda;
static u8 g_cad;
static stall_fn g_stall;
static allocate_pages_fn g_allocate_pages;

static inline void outb(u16 port, u8 value) { __asm__ volatile("outb %0, %1" :: "a"(value), "d"(port)); }
static inline u8 inb(u16 port) { u8 value; __asm__ volatile("inb %1, %0" : "=a"(value) : "d"(port)); return value; }
static inline void outl(u16 port, u32 value) { __asm__ volatile("outl %0, %1" :: "a"(value), "d"(port)); }
static inline u32 inl(u16 port) { u32 value; __asm__ volatile("inl %1, %0" : "=a"(value) : "d"(port)); return value; }
static inline void fence(void) { __asm__ volatile("mfence" ::: "memory"); }

static void serial_init(void) {
    outb(0x3f9,0); outb(0x3fb,0x80); outb(0x3f8,3); outb(0x3f9,0);
    outb(0x3fb,3); outb(0x3fa,0xc7); outb(0x3fc,0x0b);
}
static void serial_char(char ch) { u32 t=1000000; while(t-- && !(inb(0x3fd)&0x20)){} outb(0x3f8,(u8)ch); }
static void serial_puts(const char *s) { while(*s) serial_char(*s++); }
static void serial_hex8(u8 v) { static const char h[]="0123456789ABCDEF"; serial_char(h[(v>>4)&15]); serial_char(h[v&15]); }
static void marker(const char *s) { serial_puts(s); serial_puts("\r\n"); }
static void serial_text(void) { serial_puts("HII_PROMPT_TEXT="); for(u8 i=0;i<g_text_len;++i) serial_char(g_text[i]); serial_puts("\r\n"); }

static u16 rd16(const u8 *p) { return (u16)p[0] | ((u16)p[1]<<8); }
static u32 rd24(const u8 *p) { return (u32)p[0] | ((u32)p[1]<<8) | ((u32)p[2]<<16); }
static u32 rd32(const u8 *p) { return (u32)p[0] | ((u32)p[1]<<8) | ((u32)p[2]<<16) | ((u32)p[3]<<24); }
static char lower_ascii(u32 ch) { if(ch>='A' && ch<='Z') ch+=32; return (char)ch; }
static void text_reset(void) { g_text_len=0; g_text[0]=0; }
static void text_add(u32 ch) {
    char c=lower_ascii(ch);
    if(c<'a' || c>'z' || g_text_len>=MAX_TEXT) return;
    g_text[g_text_len++]=c; g_text[g_text_len]=0;
}

static int copy_scsu_text(const u8 *p, const u8 *end) {
    text_reset();
    while(p<end && *p) { text_add(*p++); }
    return p<end && g_text_len>0;
}
static int copy_ucs2_text(const u8 *p, const u8 *end) {
    text_reset();
    while(p+1<end) { u16 ch=rd16(p); if(!ch) return g_text_len>0; text_add(ch); p+=2; }
    return 0;
}

static int resolve_string_package(const u8 *pkg, u32 pkg_len, u16 wanted) {
    if(pkg_len<0x2fu) return 0;
    u32 info=rd32(pkg+8);
    if(info<0x2fu || info>=pkg_len) return 0;
    const u8 *end=pkg+pkg_len;
    u16 target=wanted;
    for(u8 restart=0; restart<4; ++restart) {
        const u8 *p=pkg+info;
        u32 sid=1;
        int do_restart=0;
        while(p<end) {
            u8 type=*p;
            if(type==0x00) break;
            if(type==0x10 || type==0x11) {
                u32 skip=(type==0x11)?2u:1u;
                if(p+skip>=end) return 0;
                const u8 *s=p+skip; const u8 *q=s;
                while(q<end && *q) ++q;
                if(q>=end) return 0;
                if(sid==target) return copy_scsu_text(s,end);
                ++sid; p=q+1; continue;
            }
            if(type==0x14 || type==0x15) {
                u32 skip=(type==0x15)?2u:1u;
                if(p+skip+1>=end) return 0;
                const u8 *s=p+skip; const u8 *q=s;
                while(q+1<end && rd16(q)) q+=2;
                if(q+1>=end) return 0;
                if(sid==target) return copy_ucs2_text(s,end);
                ++sid; p=q+2; continue;
            }
            if(type==0x12 || type==0x13) {
                u32 hdr=(type==0x13)?4u:3u;
                if(p+hdr>end) return 0;
                u16 count=rd16(p + ((type==0x13)?2:1)); p+=hdr;
                for(u16 i=0;i<count;++i) {
                    const u8 *s=p; while(p<end && *p) ++p; if(p>=end) return 0;
                    if(sid==target) return copy_scsu_text(s,end);
                    ++sid; ++p;
                }
                continue;
            }
            if(type==0x16 || type==0x17) {
                u32 hdr=(type==0x17)?4u:3u;
                if(p+hdr>end) return 0;
                u16 count=rd16(p + ((type==0x17)?2:1)); p+=hdr;
                for(u16 i=0;i<count;++i) {
                    const u8 *s=p; while(p+1<end && rd16(p)) p+=2; if(p+1>=end) return 0;
                    if(sid==target) return copy_ucs2_text(s,end);
                    ++sid; p+=2;
                }
                continue;
            }
            if(type==0x20) {
                if(p+3>end) return 0;
                if(sid==target) { target=rd16(p+1); if(!target) return 0; do_restart=1; break; }
                ++sid; p+=3; continue;
            }
            if(type==0x21) { if(p+3>end) return 0; sid+=rd16(p+1); p+=3; continue; }
            if(type==0x22) { if(p+2>end) return 0; sid+=p[1]; p+=2; continue; }
            if(type==0x30) { if(p+3>end) return 0; u32 n=p[2]; if(n<3 || p+n>end) return 0; p+=n; continue; }
            if(type==0x31) { if(p+4>end) return 0; u32 n=rd16(p+2); if(n<4 || p+n>end) return 0; p+=n; continue; }
            if(type==0x32) { if(p+6>end) return 0; u32 n=rd32(p+2); if(n<6 || p+n>end) return 0; p+=n; continue; }
            return 0;
        }
        if(!do_restart) return 0;
    }
    return 0;
}

static int prompt_opcode(u8 op) {
    switch(op) {
        case 0x02: case 0x03: case 0x05: case 0x06: case 0x07: case 0x08:
        case 0x0c: case 0x0d: case 0x0f: case 0x1a: case 0x1b: case 0x1c: case 0x23:
            return 1;
        default: return 0;
    }
}

static int scan_package_list(const u8 *list, u32 list_len) {
    if(list_len<20u) return 0;
    struct pkg_ref forms[MAX_FORMS], strings[MAX_STRINGS];
    u8 nf=0, ns=0;
    const u8 *p=list+20, *end=list+list_len;
    while(p+4<=end) {
        u32 len=rd24(p); u8 type=p[3];
        if(len<4 || p+len>end) return 0;
        if(type==0xdf) break;
        if(type==0x02 && nf<MAX_FORMS) { forms[nf].p=p; forms[nf++].len=len; }
        if(type==0x04 && ns<MAX_STRINGS) { strings[ns].p=p; strings[ns++].len=len; }
        p+=len;
    }
    if(!nf || !ns) return 0;
    marker("HII_FORMS_PACKAGE=PASS"); marker("HII_STRINGS_PACKAGE=PASS");
    for(u8 fi=0;fi<nf;++fi) {
        const u8 *q=forms[fi].p+4, *fend=forms[fi].p+forms[fi].len;
        while(q+2<=fend) {
            u8 op=q[0], len=(u8)(q[1]&0x7f);
            if(len<2 || q+len>fend) break;
            if(prompt_opcode(op) && len>=4) {
                u16 token=rd16(q+2);
                if(token) {
                    for(u8 si=0;si<ns;++si) {
                        if(resolve_string_package(strings[si].p, strings[si].len, token)) {
                            marker("IFR_PROMPT_STRING_ID=PASS"); marker("HII_PROMPT_SOURCE=PASS"); serial_text(); return 1;
                        }
                    }
                }
            }
            q+=len;
        }
    }
    return 0;
}

static int capture_live_hii(void *boot_services) {
    if(!boot_services) return 0;
    locate_protocol_fn locate=*(locate_protocol_fn *)((u8*)boot_services+0x140);
    if(!locate) return 0;
    void *db=0;
    if(locate(&g_hii_db_guid,0,&db)!=0 || !db) return 0;
    marker("HII_DATABASE_PROTOCOL=PASS");
    hii_export_fn export_all=*(hii_export_fn *)((u8*)db+0x20);
    if(!export_all) return 0;
    usize size=HII_BUFFER_BYTES;
    if(export_all(db,0,&size,g_hii_buffer)!=0 || size<20 || size>HII_BUFFER_BYTES) return 0;
    marker("HII_EXPORT_ALL_PACKAGE_LISTS=PASS");
    const u8 *p=g_hii_buffer, *end=g_hii_buffer+size;
    while(p+20<=end) {
        u32 len=rd32(p+16);
        if(len<20 || p+len>end) return 0;
        if(scan_package_list(p,len)) return 1;
        p+=len;
    }
    return 0;
}

static u32 pci_read32(u32 cfg) { outl(0xcf8,cfg); return inl(0xcfc); }
static void pci_write32(u32 cfg,u32 value) { outl(0xcf8,cfg); outl(0xcfc,value); }
static inline u16 mmio16(u32 off) { return *(volatile u16 *)(g_hda+off); }
static inline u32 mmio32(u32 off) { return *(volatile u32 *)(g_hda+off); }
static inline void mmio16w(u32 off,u16 value) { *(volatile u16 *)(g_hda+off)=value; fence(); }
static inline void mmio32w(u32 off,u32 value) { *(volatile u32 *)(g_hda+off)=value; fence(); }

static u32 immediate(u32 command) {
    u32 t=100000; while(t-- && (mmio16(0x68)&1)){} if(!t) return INVALID_RESP;
    mmio16w(0x68,2); mmio32w(0x60,command); mmio16w(0x68,1);
    t=100000; while(t-- && !(mmio16(0x68)&2)){} if(!t) return INVALID_RESP;
    u32 r=mmio32(0x64); mmio16w(0x68,2); return r;
}
static u32 encode_verb12(u8 cad,u8 nid,u16 verb,u8 payload) { return ((u32)cad<<28)|((u32)nid<<20)|((u32)verb<<8)|payload; }
static u32 encode_verb4(u8 cad,u8 nid,u8 verb,u16 payload) { return ((u32)cad<<28)|((u32)nid<<20)|((u32)(verb&15)<<16)|payload; }
static u32 verb12(u8 nid,u16 verb,u8 payload) { return immediate(encode_verb12(g_cad,nid,verb,payload)); }
static u32 verb4(u8 nid,u8 verb,u16 payload) { return immediate(encode_verb4(g_cad,nid,verb,payload)); }
static u32 get_param(u8 nid,u8 param) { return verb12(nid,0xf00,param); }
static u32 get_conn_entry(u8 nid,u8 index) { return verb12(nid,0xf02,index); }
static u32 get_conn_select(u8 nid) { return verb12(nid,0xf01,0); }
static u32 set_conn_select(u8 nid,u8 index) { return verb12(nid,0x701,index); }

static void clear_graph(void) {
    for(u32 i=0;i<MAX_NID;++i) {
        g_type[i]=0xff; g_conn_count[i]=0; g_pin_output[i]=0; g_seen[i]=0; g_parent[i]=INVALID_NID; g_depth[i]=0; g_queue[i]=0; g_route_index[i]=0;
        for(u32 j=0;j<MAX_CONN;++j) g_conn[i][j]=0;
    }
}
static int add_conn(u8 node,u16 nid) {
    if(!nid || nid>=MAX_NID) return 0;
    u8 n=g_conn_count[node]; if(n>=MAX_CONN) return 0;
    g_conn[node][n]=(u8)nid; g_conn_count[node]=(u8)(n+1); return 1;
}
static int decode_connections(u8 node) {
    u32 par=get_param(node,0x0e); if(par==INVALID_RESP) return 0; u8 raw=(u8)(par&0x7f); if(!raw) return 1;
    int lf=!!(par&0x80); u8 per=lf?2:4; u16 mask=lf?0x7fff:0x007f, range=lf?0x8000:0x0080, prev=0; int have=0, prev_range=0;
    for(u8 base=0;base<raw;base=(u8)(base+per)) {
        u32 resp=get_conn_entry(node,base); if(resp==INVALID_RESP) return 0;
        for(u8 slot=0;slot<per;++slot) {
            if((u8)(base+slot)>=raw) break;
            u16 value=lf?(u16)((resp>>(slot*16))&0xffff):(u16)((resp>>(slot*8))&0xff); u16 nid=value&mask; int is_range=!!(value&range);
            if(!nid) return 0;
            if(is_range) { if(!have || prev_range || prev>=nid) return 0; for(u16 x=(u16)(prev+1);x<=nid;++x) if(!add_conn(node,x)) return 0; }
            else if(!add_conn(node,nid)) return 0;
            prev=nid; have=1; prev_range=is_range;
        }
    }
    return 1;
}
static int traversable(u8 type) { return type==WIDGET_MIXER || type==WIDGET_SELECTOR || type==WIDGET_POWER || type==WIDGET_VOLUME || type==WIDGET_VENDOR; }
static int selectable(u8 type) { return type==WIDGET_AUDIO_INPUT || type==WIDGET_SELECTOR || type==WIDGET_PIN || type==WIDGET_VENDOR; }
static int find_route(u8 pin,u8 *dac_out,u8 *selectors_out) {
    for(u32 i=0;i<MAX_NID;++i){ g_seen[i]=0; g_parent[i]=INVALID_NID; g_depth[i]=0; g_route_index[i]=0; }
    u16 head=0,tail=0; g_queue[tail++]=pin; g_seen[pin]=1; u8 found=INVALID_NID;
    while(head<tail) {
        u8 node=g_queue[head++]; if(g_depth[node]>=16) continue;
        for(u8 ci=0;ci<g_conn_count[node];++ci) {
            u8 up=g_conn[node][ci]; if(g_seen[up]) continue; u8 type=g_type[up]; if(type==0xff) continue;
            g_seen[up]=1; g_parent[up]=node; g_route_index[up]=ci; g_depth[up]=(u8)(g_depth[node]+1);
            if(type==WIDGET_AUDIO_OUTPUT){ found=up; head=tail; break; }
            if(traversable(type) && tail<MAX_NID) g_queue[tail++]=up;
        }
    }
    if(found==INVALID_NID) return 0; u8 selectors=0,cur=found;
    while(cur!=pin) {
        u8 child=g_parent[cur]; if(child==INVALID_NID) return 0;
        if(g_conn_count[child]>1){ u8 type=g_type[child]; if(type==WIDGET_MIXER){} else if(selectable(type)) ++selectors; else return 0; }
        cur=child;
    }
    *dac_out=found; *selectors_out=selectors; return 1;
}
static int apply_route(u8 pin,u8 dac,u8 *applied_out) {
    u8 applied=0,cur=dac;
    while(cur!=pin) {
        u8 child=g_parent[cur]; if(child==INVALID_NID) return 0;
        if(g_conn_count[child]>1){
            u8 type=g_type[child];
            if(type==WIDGET_MIXER){}
            else if(selectable(type)){
                u8 idx=g_route_index[cur]; if(set_conn_select(child,idx)==INVALID_RESP) return 0;
                u32 r=get_conn_select(child); if(r==INVALID_RESP || (u8)r!=idx) return 0; ++applied;
            } else return 0;
        }
        cur=child;
    }
    *applied_out=applied; return 1;
}
static int configure_output_path(u8 pin,u8 dac) {
    u32 amp=get_param(dac,0x12); if(amp==INVALID_RESP) return 0;
    if(amp){
        if(verb4(dac,0x3,0xb040)==INVALID_RESP) return 0;
        u32 l=verb4(dac,0xb,0xa000), r=verb4(dac,0xb,0x8000);
        if(l==INVALID_RESP||r==INVALID_RESP) return 0;
    }
    u32 pc=get_param(pin,0x0c); if(pc==INVALID_RESP) return 0;
    if(pc&0x00010000u){
        if(verb12(pin,0x70c,0x02)==INVALID_RESP) return 0;
        u32 e=verb12(pin,0xf0c,0); if(e==INVALID_RESP || !(e&2)) return 0;
    }
    if(verb12(dac,0x706,0x10)==INVALID_RESP) return 0;
    if(verb4(dac,0x2,0x0011)==INVALID_RESP) return 0;
    if(verb12(pin,0x707,0x40)==INVALID_RESP) return 0;
    return 1;
}
static int graph_selftest(void) {
    clear_graph(); g_type[0x14]=WIDGET_PIN; g_type[0x0c]=WIDGET_MIXER; g_type[0x0b]=WIDGET_SELECTOR; g_type[0x02]=WIDGET_AUDIO_OUTPUT; g_type[0x03]=0xff;
    g_conn_count[0x14]=1; g_conn[0x14][0]=0x0c; g_conn_count[0x0c]=1; g_conn[0x0c][0]=0x0b; g_conn_count[0x0b]=2; g_conn[0x0b][0]=0x03; g_conn[0x0b][1]=0x02;
    u8 d=0,s=0;
    return find_route(0x14,&d,&s) && d==0x02 && s==1 && g_depth[d]==3 && g_route_index[d]==1 && encode_verb12(2,0x14,0x701,3)==0x21470103u;
}
static int discover_controller(void) {
    u32 cfg=0; int found=0;
    for(u32 bdf=0;bdf<0x10000;++bdf){
        u32 base=0x80000000u|(bdf<<8), vd=pci_read32(base);
        if((vd&0xffff)==0xffff) continue;
        u32 cr=pci_read32(base|0x08); u16 cls=(u16)((cr>>16)&0xffff);
        if(cls==0x0403 || cls==0x0401){ cfg=base; found=1; break; }
    }
    if(!found) return 0;
    u32 cmd=pci_read32(cfg|0x04); pci_write32(cfg|0x04,cmd|6u);
    u32 bar0=pci_read32(cfg|0x10); if(bar0&1) return 0;
    u64 bar=(u64)(bar0&0xfffffff0u); if((bar0&6)==4) bar|=((u64)pci_read32(cfg|0x14))<<32; if(!bar) return 0;
    g_hda=(volatile u8 *)(usize)bar; if(!mmio16(0x00) || !*(volatile u8 *)(g_hda+0x03)) return 0;
    u32 gctl=mmio32(0x08); mmio32w(0x08,gctl&~1u);
    u32 t=100000; while(t-- && (mmio32(0x08)&1)){} if(!t) return 0; if(g_stall) g_stall(100);
    mmio32w(0x08,mmio32(0x08)|1u); t=100000; while(t-- && !(mmio32(0x08)&1)){} if(!t) return 0; if(g_stall) g_stall(1000);
    u16 state=mmio16(0x0e); if(!state) return 0;
    for(u8 cad=0;cad<15;++cad) if(state&(1u<<cad)){ g_cad=cad; return 1; }
    return 0;
}
static int discover_live_graph(u8 *pin_out,u8 *dac_out,u8 *selectors_out) {
    clear_graph();
    u32 roots=get_param(0,0x04); if(roots==INVALID_RESP) return 0;
    u8 rs=(u8)((roots>>16)&0xff), rc=(u8)(roots&0xff); if(!rc) return 0; u8 afg=INVALID_NID;
    for(u16 n=rs;n<(u16)rs+rc;++n){
        u32 type=get_param((u8)n,0x05);
        if(type!=INVALID_RESP && (type&0xff)==1){ afg=(u8)n; break; }
    }
    if(afg==INVALID_NID) return 0;
    marker("HDA_AFG_RUNTIME=PASS");
    u32 widgets=get_param(afg,0x04); if(widgets==INVALID_RESP) return 0;
    u8 start=(u8)((widgets>>16)&0xff), count=(u8)(widgets&0xff); if(!count) return 0;
    for(u16 n=start;n<(u16)start+count;++n){
        u8 nid=(u8)n; u32 cap=get_param(nid,0x09); if(cap==INVALID_RESP) return 0;
        u8 type=(u8)((cap>>20)&15); g_type[nid]=type;
        if(type==WIDGET_PIN){
            u32 pc=get_param(nid,0x0c); if(pc==INVALID_RESP) return 0;
            if(pc&0x10) g_pin_output[nid]=1;
        }
        if(!decode_connections(nid)) return 0;
    }
    marker("HDA_WIDGET_ENUMERATION=PASS"); marker("HDA_CONNECTION_LIST_DECODE=PASS");
    for(u16 n=start;n<(u16)start+count;++n){
        u8 pin=(u8)n; if(!g_pin_output[pin]) continue;
        u8 dac=0,sel=0;
        if(find_route(pin,&dac,&sel)){ *pin_out=pin; *dac_out=dac; *selectors_out=sel; return 1; }
    }
    return 0;
}
static void copy_bytes(volatile u8 *dst,const u8 *src,u32 len){ for(u32 i=0;i<len;++i) dst[i]=src[i]; }
static int run_text_dma(void) {
    if(!g_allocate_pages || !g_text_len || !qev_unit_bank_len || qev_unit_count==0 || qev_letter_stride==0) return 0;
    u64 base=0xffffffffu;
    if(g_allocate_pages(1,4,DMA_PAGES,&base)!=0 || !base || base>0xffffffffu) return 0;
    if(qev_unit_bank_len>DMA_PAGES*4096u-PCM_OFF) return 0;
    volatile u8 *pcm=(volatile u8 *)(usize)(base+PCM_OFF);
    copy_bytes(pcm,qev_unit_bank,qev_unit_bank_len);
    volatile u8 *bdl=(volatile u8 *)(usize)base;
    u32 desc=0,total=0;
    for(u8 ti=0;ti<g_text_len;++ti){
        u8 li=(u8)(g_text[ti]-'a'); if(li>=26) continue;
        u8 cnt=qev_letter_count[li];
        for(u8 j=0;j<cnt;++j){
            if(desc>=MAX_BDL) return 0;
            u8 ui=qev_letter_units[(u32)li*qev_letter_stride+j]; if(ui>=qev_unit_count) return 0;
            u32 off=qev_unit_offsets[ui], len=qev_unit_lengths[ui]; if(!len || off+len>qev_unit_bank_len) return 0;
            volatile u8 *d=bdl+desc*16u;
            *(volatile u64 *)(d+0)=base+PCM_OFF+off;
            *(volatile u32 *)(d+8)=len;
            *(volatile u32 *)(d+12)=0;
            total+=len; ++desc;
        }
    }
    if(!desc || !total) return 0;
    *(volatile u32 *)(bdl+(desc-1u)*16u+12)=1; fence();
    u16 gcap=mmio16(0x00); u8 iss=(u8)((gcap>>8)&15);
    volatile u8 *sd=g_hda+0x80+((u32)iss*0x20);
    sd[0]=(u8)(sd[0]&~2u);
    u32 t=100000; while(t-- && (sd[0]&2)){} if(!t) return 0;
    sd[0]=(u8)(sd[0]|1u); t=100000; while(t-- && !(sd[0]&1)){} if(!t) return 0;
    sd[0]=(u8)(sd[0]&~1u); t=100000; while(t-- && (sd[0]&1)){} if(!t) return 0;
    *(volatile u32 *)(sd+0x08)=total;
    *(volatile u16 *)(sd+0x0c)=(u16)(desc-1u);
    *(volatile u16 *)(sd+0x12)=0x0011;
    *(volatile u32 *)(sd+0x18)=(u32)base;
    *(volatile u32 *)(sd+0x1c)=(u32)(base>>32);
    fence();
    sd[2]=0x10; sd[0]=(u8)(sd[0]|2u);
    usize play_us=((usize)total*1000000u)/(48000u*4u)+150000u;
    if(play_us>8000000u) play_us=8000000u;
    if(g_stall) g_stall(play_us);
    u32 lpib=*(volatile u32 *)(sd+0x04);
    sd[0]=(u8)(sd[0]&~2u);
    return lpib!=0;
}

__attribute__((ms_abi)) u64 efi_main(void *image_handle, void *system_table) {
    (void)image_handle;
    serial_init();
    marker("QEVARYNOX-UEFI-HII-GRAPH-SPEECH-V1");
    marker("STATE=START");
    marker("FRAMEWORK=NONE");
    marker("EDK2=NONE");
    marker("NO_FIXED_WIDGET_NIDS=PASS");
    if(!system_table){ marker("STATUS=BLOCKED"); marker("REASON=SYSTEM_TABLE_MISSING"); return 1; }
    void *bs=*(void **)((u8*)system_table+0x60);
    if(!bs){ marker("STATUS=BLOCKED"); marker("REASON=BOOT_SERVICES_MISSING"); return 1; }
    g_allocate_pages=*(allocate_pages_fn *)((u8*)bs+0x28);
    g_stall=*(stall_fn *)((u8*)bs+0xf8);

    if(!capture_live_hii(bs)){ marker("STATUS=BLOCKED"); marker("REASON=LIVE_HII_PROMPT_NOT_RESOLVED"); return 1; }
    marker("HII_TO_SYNTH_TEXT=PASS");

    if(!graph_selftest()){ marker("STATUS=BLOCKED"); marker("REASON=HDA_MULTIHOP_SELFTEST_FAILED"); return 1; }
    marker("HDA_MULTIHOP_EFI_SELFTEST=PASS");
    if(!discover_controller()){ marker("STATUS=BLOCKED"); marker("REASON=HDA_CONTROLLER_OR_CODEC_NOT_FOUND"); return 1; }
    marker("HDA_CONTROLLER_CODEC=PASS");

    u8 pin=0,dac=0,selectors=0;
    if(!discover_live_graph(&pin,&dac,&selectors)){ marker("STATUS=BLOCKED"); marker("REASON=HDA_GRAPH_ROUTE_NOT_FOUND"); return 1; }
    marker("HDA_GRAPH_SEARCH_LIVE=PASS");
    u8 applied=0;
    if(!apply_route(pin,dac,&applied) || applied!=selectors){ marker("STATUS=BLOCKED"); marker("REASON=HDA_SELECTOR_APPLY_OR_READBACK_FAILED"); return 1; }
    marker("HDA_SELECTOR_APPLY_LIVE=PASS");
    serial_puts("HDA_PIN_NID=0x"); serial_hex8(pin); serial_puts("\r\n");
    serial_puts("HDA_DAC_NID=0x"); serial_hex8(dac); serial_puts("\r\n");
    serial_puts("HDA_ROUTE_DEPTH=0x"); serial_hex8(g_depth[dac]); serial_puts("\r\n");

    if(!configure_output_path(pin,dac)){ marker("STATUS=BLOCKED"); marker("REASON=HDA_OUTPUT_PATH_CONFIGURATION_FAILED"); return 1; }
    marker("HDA_OUTPUT_PATH_CONFIGURATION=PASS");
    marker("SYNTH=ALLOPHONE_BDL_RUNTIME_V1");
    marker("SYNTH_SOURCE=LIVE_HII_PROMPT");

    if(!run_text_dma()){ marker("STATUS=BLOCKED"); marker("REASON=HII_GRAPH_DMA_SPEECH_FAILED"); return 1; }
    marker("LPIB_PROGRESS=PASS");
    marker("HII_PROMPT_SPEECH_HDA=PASS");
    marker("HII_MULTIHOP_SPEECH_INTEGRATION=PASS");
    marker("STATUS=PASS");
    return 0;
}
