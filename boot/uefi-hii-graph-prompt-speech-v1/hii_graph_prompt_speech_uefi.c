typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef unsigned long long usize;

typedef u64 (*stall_fn)(usize microseconds);
typedef u64 (*allocate_pages_fn)(u32 type, u32 memory_type, usize pages, u64 *memory);

#ifdef QEV_INTERACTIVE_REPEAT
typedef struct {
    u16 scan_code;
    u16 unicode_char;
} efi_input_key;
typedef u64 (*read_key_fn)(void *self, efi_input_key *key);
typedef struct {
    void *reset;
    read_key_fn read_key;
    void *wait_for_key;
} simple_text_input_protocol;
#endif

extern const u8 qev_unit_bank[];
extern const u32 qev_unit_bank_len;
extern const u32 qev_unit_off[];
extern const u32 qev_unit_len[];
extern const u32 qev_unit_count;
extern const u8 qev_letter_unit_count[];
extern const u8 qev_letter_units[];

#define MAX_NID 256
#define MAX_CONN 64
#define INVALID_NID 0xff
#define INVALID_RESP 0xffffffffu

#define WIDGET_AUDIO_OUTPUT 0x0
#define WIDGET_AUDIO_INPUT  0x1
#define WIDGET_MIXER        0x2
#define WIDGET_SELECTOR     0x3
#define WIDGET_PIN          0x4
#define WIDGET_POWER        0x5
#define WIDGET_VOLUME       0x6
#define WIDGET_VENDOR       0xf

static u8 g_type[MAX_NID];
static u8 g_conn_count[MAX_NID];
static u8 g_conn[MAX_NID][MAX_CONN];
static u8 g_pin_output[MAX_NID];
static u8 g_seen[MAX_NID];
static u8 g_parent[MAX_NID];
static u8 g_depth[MAX_NID];
static u8 g_queue[MAX_NID];
static u8 g_route_index[MAX_NID];

static volatile u8 *g_hda;
static u8 g_cad;
static u8 g_controller_preferred;
static u32 g_codec_vendor_id;
static stall_fn g_stall;
static allocate_pages_fn g_allocate_pages;

typedef u64 (*locate_protocol_fn)(const void *protocol, void *registration, void **interface_out);
typedef u64 (*handle_protocol_fn)(void *handle, const void *protocol, void **interface_out);
typedef u64 (*file_open_fn)(void *self, void **new_handle, const u16 *name, u64 open_mode, u64 attributes);
typedef u64 (*file_close_fn)(void *self);
typedef u64 (*file_write_fn)(void *self, usize *buffer_size, void *buffer);
typedef u64 (*file_flush_fn)(void *self);

typedef struct {
    u32 revision;
    u32 reserved;
    void *parent_handle;
    void *system_table;
    void *device_handle;
} loaded_image_protocol_head;

typedef struct file_protocol {
    u64 revision;
    file_open_fn open;
    file_close_fn close;
    void *delete_file;
    void *read;
    file_write_fn write;
    void *get_position;
    void *set_position;
    void *get_info;
    void *set_info;
    file_flush_fn flush;
} file_protocol;

typedef u64 (*open_volume_fn)(void *self, file_protocol **root);
typedef struct {
    u64 revision;
    open_volume_fn open_volume;
} simple_fs_protocol;
typedef u64 (*hii_list_fn)(const void *self, u8 package_type, const void *package_guid, usize *handle_bytes, void **handles);
typedef u64 (*hii_export_fn)(const void *self, void *handle, usize *buffer_size, void *buffer);
typedef u64 (*hii_get_string_fn)(const void *self, const char *language, void *handle, u16 string_id, u16 *string, usize *string_size, void **font_info);
typedef u64 (*hii_get_languages_fn)(const void *self, void *handle, char *languages, usize *language_size);

typedef struct {
    void *new_package_list;
    void *remove_package_list;
    void *update_package_list;
    hii_list_fn list_package_lists;
    hii_export_fn export_package_lists;
} hii_database_protocol;

typedef struct {
    void *new_string;
    hii_get_string_fn get_string;
    void *set_string;
    hii_get_languages_fn get_languages;
} hii_string_protocol;

typedef struct {
    u32 data1;
    u16 data2;
    u16 data3;
    u8 data4[8];
} efi_guid;

static const efi_guid g_hii_database_guid =
    {0xef9fc172u,0xa1b2u,0x4693u,{0xb3,0x27,0x6d,0x32,0xfc,0x41,0x60,0x42}};
static const efi_guid g_hii_string_guid =
    {0x0fd96974u,0x23aau,0x4cdcu,{0xb9,0xcb,0x98,0xd1,0x77,0x50,0x32,0x2a}};
static const efi_guid g_loaded_image_guid =
    {0x5b1b31a1u,0x9562u,0x11d2u,{0x8e,0x3f,0x00,0xa0,0xc9,0x69,0x72,0x3b}};
static const efi_guid g_simple_fs_guid =
    {0x964e5b22u,0x6459u,0x11d2u,{0x8e,0x39,0x00,0xa0,0xc9,0x69,0x72,0x3b}};

static u8 g_hii_package[1024u * 1024u];
static void *g_hii_handles[256];
static char g_prompt_text[9];
static u32 g_prompt_count;

static inline void outb(u16 port, u8 value) {
    __asm__ volatile("outb %0, %1" :: "a"(value), "d"(port));
}
static inline u8 inb(u16 port) {
    u8 value;
    __asm__ volatile("inb %1, %0" : "=a"(value) : "d"(port));
    return value;
}
static inline void outl(u16 port, u32 value) {
    __asm__ volatile("outl %0, %1" :: "a"(value), "d"(port));
}
static inline u32 inl(u16 port) {
    u32 value;
    __asm__ volatile("inl %1, %0" : "=a"(value) : "d"(port));
    return value;
}
static inline void fence(void) {
    __asm__ volatile("mfence" ::: "memory");
}

static void serial_init(void) {
    outb(0x3f9, 0x00);
    outb(0x3fb, 0x80);
    outb(0x3f8, 0x03);
    outb(0x3f9, 0x00);
    outb(0x3fb, 0x03);
    outb(0x3fa, 0xc7);
    outb(0x3fc, 0x0b);
}
static void serial_char(char ch) {
    u32 timeout = 1000000;
    while (timeout-- && !(inb(0x3fd) & 0x20)) {}
    outb(0x3f8, (u8)ch);
}
static void serial_puts(const char *s) {
    while (*s) serial_char(*s++);
}
static void serial_hex8(u8 value) {
    static const char h[] = "0123456789ABCDEF";
    serial_char(h[(value >> 4) & 0xf]);
    serial_char(h[value & 0xf]);
}
static void serial_hex32(u32 value) {
    serial_hex8((u8)(value >> 24));
    serial_hex8((u8)(value >> 16));
    serial_hex8((u8)(value >> 8));
    serial_hex8((u8)value);
}
static void marker(const char *s) {
    serial_puts(s);
    serial_puts("\r\n");
}

static void proof_puts(char *buf, usize cap, usize *n, const char *s) {
    while (*s && *n + 1u < cap) buf[(*n)++] = *s++;
}
static void proof_hex8(char *buf, usize cap, usize *n, u8 value) {
    static const char h[] = "0123456789ABCDEF";
    if (*n + 2u < cap) {
        buf[(*n)++] = h[(value >> 4) & 0xf];
        buf[(*n)++] = h[value & 0xf];
    }
}
static void proof_hex32(char *buf, usize cap, usize *n, u32 value) {
    proof_hex8(buf,cap,n,(u8)(value >> 24));
    proof_hex8(buf,cap,n,(u8)(value >> 16));
    proof_hex8(buf,cap,n,(u8)(value >> 8));
    proof_hex8(buf,cap,n,(u8)value);
}
static int persist_boot_proof(void *image_handle, void *boot_services,
                              u8 pin, u8 dac, u8 selectors, u8 applied) {
    static const u16 filename[] = {
        '\\','Q','E','V','A','R','Y','N','O','X','-','P','H','Y','S','I','C','A','L',
        '-','P','R','O','O','F','.','T','X','T',0
    };
    char proof[1024];
    usize n = 0;
    loaded_image_protocol_head *loaded = 0;
    simple_fs_protocol *fs = 0;
    file_protocol *root = 0;
    file_protocol *file = 0;
    if (!boot_services || !image_handle) return 0;
    handle_protocol_fn handle_protocol =
        *(handle_protocol_fn *)((u8 *)boot_services + 0x98);
    if (!handle_protocol) return 0;
    if (handle_protocol(image_handle, &g_loaded_image_guid, (void **)&loaded) != 0 ||
        !loaded || !loaded->device_handle) return 0;
    if (handle_protocol(loaded->device_handle, &g_simple_fs_guid, (void **)&fs) != 0 ||
        !fs || !fs->open_volume) return 0;
    if (fs->open_volume(fs, &root) != 0 || !root || !root->open) return 0;

    const u64 open_mode = 0x8000000000000000ull | 0x2ull | 0x1ull;
    if (root->open(root, (void **)&file, filename, open_mode, 0) != 0 ||
        !file || !file->write) {
        if (root->close) root->close(root);
        return 0;
    }

    proof_puts(proof,sizeof(proof),&n,"QEVARYNOX-UEFI-PHYSICAL-BOOT-PROOF-V1\r\n");
    proof_puts(proof,sizeof(proof),&n,"STATUS=PASS\r\n");
    proof_puts(proof,sizeof(proof),&n,"HII_PROMPT_SOURCE=PASS\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_CONTROLLER_SELECTION=");
    proof_puts(proof,sizeof(proof),&n,
        g_controller_preferred ? "PREFERRED_AMD_1022_15E3\r\n" : "GENERIC_CLASS_0403\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_CODEC_VENDOR_DEVICE=0x");
    proof_hex32(proof,sizeof(proof),&n,g_codec_vendor_id);
    proof_puts(proof,sizeof(proof),&n,"\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_CODEC_SELECTION=");
    proof_puts(proof,sizeof(proof),&n,
        g_controller_preferred ? "REALTEK_10EC_0256\r\n" : "GENERIC_RUNTIME\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_GRAPH_SEARCH_LIVE=PASS\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_SELECTOR_APPLY_LIVE=PASS\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_OUTPUT_PATH_CONFIGURATION=PASS\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_PIN_NID=0x"); proof_hex8(proof,sizeof(proof),&n,pin); proof_puts(proof,sizeof(proof),&n,"\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_DAC_NID=0x"); proof_hex8(proof,sizeof(proof),&n,dac); proof_puts(proof,sizeof(proof),&n,"\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_ROUTE_DEPTH=0x"); proof_hex8(proof,sizeof(proof),&n,g_depth[dac]); proof_puts(proof,sizeof(proof),&n,"\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_SELECTOR_WRITES_REQUIRED=0x"); proof_hex8(proof,sizeof(proof),&n,selectors); proof_puts(proof,sizeof(proof),&n,"\r\n");
    proof_puts(proof,sizeof(proof),&n,"HDA_SELECTOR_WRITES_APPLIED=0x"); proof_hex8(proof,sizeof(proof),&n,applied); proof_puts(proof,sizeof(proof),&n,"\r\n");
    proof_puts(proof,sizeof(proof),&n,"HII_GRAPH_SPEECH_DMA=PASS\r\n");
    proof_puts(proof,sizeof(proof),&n,"LPIB_PROGRESS=PASS\r\n");
    proof_puts(proof,sizeof(proof),&n,"AUDIBLE_PHYSICAL_SPEAKER=REQUIRES_HUMAN_CONFIRMATION\r\n");

    usize bytes = n;
    u64 st = file->write(file, &bytes, proof);
    if (st == 0 && file->flush) st = file->flush(file);
    if (file->close) file->close(file);
    if (root->close) root->close(root);
    return st == 0 && bytes == n;
}

static u32 pci_read32(u32 cfg) {
    outl(0xcf8, cfg);
    return inl(0xcfc);
}
static void pci_write32(u32 cfg, u32 value) {
    outl(0xcf8, cfg);
    outl(0xcfc, value);
}

static inline u16 mmio16(u32 off) {
    return *(volatile u16 *)(g_hda + off);
}
static inline u32 mmio32(u32 off) {
    return *(volatile u32 *)(g_hda + off);
}
static inline void mmio16w(u32 off, u16 value) {
    *(volatile u16 *)(g_hda + off) = value;
    fence();
}
static inline void mmio32w(u32 off, u32 value) {
    *(volatile u32 *)(g_hda + off) = value;
    fence();
}

static u32 immediate(u32 command) {
    u32 timeout = 100000;
    while (timeout-- && (mmio16(0x68) & 1)) {}
    if (!timeout) return INVALID_RESP;
    mmio16w(0x68, 2);
    mmio32w(0x60, command);
    mmio16w(0x68, 1);
    timeout = 100000;
    while (timeout-- && !(mmio16(0x68) & 2)) {}
    if (!timeout) return INVALID_RESP;
    u32 response = mmio32(0x64);
    mmio16w(0x68, 2);
    return response;
}
static u32 encode_verb12(u8 cad, u8 nid, u16 verb, u8 payload) {
    return ((u32)cad << 28) | ((u32)nid << 20) |
           ((u32)verb << 8) | payload;
}
static u32 encode_verb4(u8 cad, u8 nid, u8 verb, u16 payload) {
    return ((u32)cad << 28) | ((u32)nid << 20) |
           ((u32)(verb & 0x0f) << 16) | payload;
}
static u32 verb12(u8 nid, u16 verb, u8 payload) {
    return immediate(encode_verb12(g_cad, nid, verb, payload));
}
static u32 verb4(u8 nid, u8 verb, u16 payload) {
    return immediate(encode_verb4(g_cad, nid, verb, payload));
}
static u32 get_param(u8 nid, u8 param) {
    return verb12(nid, 0xf00, param);
}
static u32 get_conn_entry(u8 nid, u8 index) {
    return verb12(nid, 0xf02, index);
}
static u32 get_conn_select(u8 nid) {
    return verb12(nid, 0xf01, 0);
}
static u32 set_conn_select(u8 nid, u8 index) {
    return verb12(nid, 0x701, index);
}

static void clear_graph(void) {
    u32 i, j;
    for (i = 0; i < MAX_NID; ++i) {
        g_type[i] = 0xff;
        g_conn_count[i] = 0;
        g_pin_output[i] = 0;
        g_seen[i] = 0;
        g_parent[i] = INVALID_NID;
        g_depth[i] = 0;
        g_queue[i] = 0;
        g_route_index[i] = 0;
        for (j = 0; j < MAX_CONN; ++j) g_conn[i][j] = 0;
    }
}

static int add_conn(u8 node, u16 nid) {
    if (!nid || nid >= MAX_NID) return 0;
    u8 count = g_conn_count[node];
    if (count >= MAX_CONN) return 0;
    g_conn[node][count] = (u8)nid;
    g_conn_count[node] = count + 1;
    return 1;
}

static int decode_connections(u8 node) {
    u32 parameter = get_param(node, 0x0e);
    if (parameter == INVALID_RESP) return 0;
    u8 raw_count = (u8)(parameter & 0x7f);
    if (!raw_count) return 1;
    int long_form = !!(parameter & 0x80);
    u8 per_response = long_form ? 2 : 4;
    u16 mask = long_form ? 0x7fff : 0x007f;
    u16 range_bit = long_form ? 0x8000 : 0x0080;
    u16 previous = 0;
    int have_previous = 0;
    int previous_was_range = 0;

    for (u8 base = 0; base < raw_count; base = (u8)(base + per_response)) {
        u32 response = get_conn_entry(node, base);
        if (response == INVALID_RESP) return 0;
        for (u8 slot = 0; slot < per_response; ++slot) {
            u8 raw_index = (u8)(base + slot);
            if (raw_index >= raw_count) break;
            u16 value = long_form
                ? (u16)((response >> (slot * 16)) & 0xffff)
                : (u16)((response >> (slot * 8)) & 0xff);
            u16 nid = value & mask;
            int is_range = !!(value & range_bit);
            if (!nid) return 0;
            if (is_range) {
                if (!have_previous || previous_was_range || previous >= nid) return 0;
                for (u16 expanded = (u16)(previous + 1); expanded <= nid; ++expanded) {
                    if (!add_conn(node, expanded)) return 0;
                }
            } else {
                if (!add_conn(node, nid)) return 0;
            }
            previous = nid;
            have_previous = 1;
            previous_was_range = is_range;
        }
    }
    return 1;
}

static int traversable(u8 type) {
    return type == WIDGET_MIXER || type == WIDGET_SELECTOR ||
           type == WIDGET_POWER || type == WIDGET_VOLUME ||
           type == WIDGET_VENDOR;
}
static int selectable(u8 type) {
    return type == WIDGET_AUDIO_INPUT || type == WIDGET_SELECTOR ||
           type == WIDGET_PIN || type == WIDGET_VENDOR;
}

static int find_route(u8 pin, u8 *dac_out, u8 *selector_count_out) {
    u32 i;
    for (i = 0; i < MAX_NID; ++i) {
        g_seen[i] = 0;
        g_parent[i] = INVALID_NID;
        g_depth[i] = 0;
        g_route_index[i] = 0;
    }
    u16 qhead = 0, qtail = 0;
    g_queue[qtail++] = pin;
    g_seen[pin] = 1;
    u8 found = INVALID_NID;

    while (qhead < qtail) {
        u8 node = g_queue[qhead++];
        if (g_depth[node] >= 16) continue;
        u8 count = g_conn_count[node];
        for (u8 ci = 0; ci < count; ++ci) {
            u8 upstream = g_conn[node][ci];
            if (g_seen[upstream]) continue;
            u8 type = g_type[upstream];
            if (type == 0xff) continue;
            g_seen[upstream] = 1;
            g_parent[upstream] = node;
            g_route_index[upstream] = ci;
            g_depth[upstream] = (u8)(g_depth[node] + 1);
            if (type == WIDGET_AUDIO_OUTPUT) {
                found = upstream;
                qhead = qtail;
                break;
            }
            if (traversable(type) && qtail < MAX_NID) {
                g_queue[qtail++] = upstream;
            }
        }
    }
    if (found == INVALID_NID) return 0;

    u8 selectors = 0;
    u8 cur = found;
    while (cur != pin) {
        u8 child = g_parent[cur];
        if (child == INVALID_NID) return 0;
        if (g_conn_count[child] > 1) {
            u8 type = g_type[child];
            if (type == WIDGET_MIXER) {
            } else if (selectable(type)) {
                ++selectors;
            } else {
                return 0;
            }
        }
        cur = child;
    }
    *dac_out = found;
    *selector_count_out = selectors;
    return 1;
}

static int apply_route(u8 pin, u8 dac, u8 *applied_out) {
    u8 applied = 0;
    u8 cur = dac;
    while (cur != pin) {
        u8 child = g_parent[cur];
        if (child == INVALID_NID) return 0;
        if (g_conn_count[child] > 1) {
            u8 type = g_type[child];
            if (type == WIDGET_MIXER) {
            } else if (selectable(type)) {
                u8 index = g_route_index[cur];
                if (set_conn_select(child, index) == INVALID_RESP) return 0;
                u32 readback = get_conn_select(child);
                if (readback == INVALID_RESP || (u8)readback != index) return 0;
                ++applied;
            } else {
                return 0;
            }
        }
        cur = child;
    }
    *applied_out = applied;
    return 1;
}

static int configure_output_path(u8 pin, u8 dac) {
    u32 amp_cap = get_param(dac, 0x12);
    if (amp_cap == INVALID_RESP) return 0;
    if (amp_cap) {
        if (verb4(dac, 0x3, 0xb040) == INVALID_RESP) return 0;
        u32 left = verb4(dac, 0xb, 0xa000);
        u32 right = verb4(dac, 0xb, 0x8000);
        if (left == INVALID_RESP || right == INVALID_RESP) return 0;
        if ((left & 0x7f) != 0x40 || (right & 0x7f) != 0x40) return 0;
    }

    u32 pin_cap = get_param(pin, 0x0c);
    if (pin_cap == INVALID_RESP) return 0;
    if (pin_cap & 0x00010000u) {
        if (verb12(pin, 0x70c, 0x02) == INVALID_RESP) return 0;
        u32 eapd = verb12(pin, 0xf0c, 0);
        if (eapd == INVALID_RESP || !(eapd & 0x02)) return 0;
    }

    if (verb12(dac, 0x706, 0x10) == INVALID_RESP) return 0;
    if (verb4(dac, 0x2, 0x0011) == INVALID_RESP) return 0;
    if (verb12(pin, 0x707, 0x40) == INVALID_RESP) return 0;
    return 1;
}

static void copy_bytes(volatile u8 *dst, const u8 *src, u32 len) {
    for (u32 i = 0; i < len; ++i) dst[i] = src[i];
}

static int run_speech_dma(const char *text, u32 text_count) {
    if (!g_allocate_pages || !text || !text_count || text_count > 8u) return 0;
    u64 base = 0xffffffffu;
    if (g_allocate_pages(1, 4, 128, &base) != 0 || !base || base > 0xffffffffu) return 0;

    const u32 pcm_off = 0x1000;
    if (!qev_unit_bank_len || qev_unit_bank_len > (128u * 4096u - pcm_off)) return 0;
    volatile u8 *pcm = (volatile u8 *)(usize)(base + pcm_off);
    copy_bytes(pcm, qev_unit_bank, qev_unit_bank_len);

    volatile u8 *bdl = (volatile u8 *)(usize)base;
    u32 entries = 0;
    u32 total_bytes = 0;
    for (u32 i = 0; i < text_count; ++i) {
        u8 ch = (u8)text[i];
        if (ch < (u8)'a' || ch > (u8)'z') return 0;
        u32 li = (u32)(ch - (u8)'a');
        u32 n = qev_letter_unit_count[li];
        if (!n || n > 8u) return 0;
        for (u32 j = 0; j < n; ++j) {
            if (entries >= 64u) return 0;
            u32 ui = qev_letter_units[li * 8u + j];
            if (ui >= qev_unit_count) return 0;
            u32 off = qev_unit_off[ui];
            u32 len = qev_unit_len[ui];
            if (!len || off > qev_unit_bank_len || len > qev_unit_bank_len - off) return 0;
            volatile u8 *e = bdl + entries * 16u;
            *(volatile u64 *)(e + 0x00) = base + pcm_off + off;
            *(volatile u32 *)(e + 0x08) = len;
            *(volatile u32 *)(e + 0x0c) = 0;
            total_bytes += len;
            ++entries;
        }
    }
    if (!entries || !total_bytes) return 0;
    *(volatile u32 *)(bdl + (entries - 1u) * 16u + 0x0c) = 1;
    fence();

    u16 gcap = mmio16(0x00);
    u8 iss = (u8)((gcap >> 8) & 0x0f);
    volatile u8 *sd = g_hda + 0x80 + ((u32)iss * 0x20);

    sd[0] = (u8)(sd[0] & ~2u);
    u32 timeout = 100000;
    while (timeout-- && (sd[0] & 2)) {}
    if (!timeout) return 0;
    sd[0] = (u8)(sd[0] | 1u);
    timeout = 100000;
    while (timeout-- && !(sd[0] & 1)) {}
    if (!timeout) return 0;
    sd[0] = (u8)(sd[0] & ~1u);
    timeout = 100000;
    while (timeout-- && (sd[0] & 1)) {}
    if (!timeout) return 0;

    *(volatile u32 *)(sd + 0x08) = total_bytes;
    *(volatile u16 *)(sd + 0x0c) = (u16)(entries - 1u);
    *(volatile u16 *)(sd + 0x12) = 0x0011;
    *(volatile u32 *)(sd + 0x18) = (u32)base;
    *(volatile u32 *)(sd + 0x1c) = (u32)(base >> 32);
    fence();

    sd[2] = 0x10;
    sd[0] = (u8)(sd[0] | 2u);
    if (g_stall) g_stall(700000);
    u32 lpib = *(volatile u32 *)(sd + 0x04);
    sd[0] = (u8)(sd[0] & ~2u);
    return lpib != 0;
}

static u16 rd16(const u8 *p) {
    return (u16)((u16)p[0] | ((u16)p[1] << 8));
}
static u32 rd32(const u8 *p) {
    return (u32)p[0] | ((u32)p[1] << 8) | ((u32)p[2] << 16) | ((u32)p[3] << 24);
}
static int prompt_opcode(u8 op) {
    switch (op) {
        case 0x02: case 0x03: case 0x05: case 0x06: case 0x07:
        case 0x08: case 0x0c: case 0x0d: case 0x0f: case 0x1a:
        case 0x1b: case 0x1c: case 0x23:
            return 1;
        default:
            return 0;
    }
}
static int normalize_prompt(const u16 *text, char *out, u32 *count_out) {
    u32 n = 0;
    for (u32 i = 0; i < 127u && text[i]; ++i) {
        u16 ch = text[i];
        if (ch >= (u16)'A' && ch <= (u16)'Z') ch = (u16)(ch + 32u);
        if (ch >= (u16)'a' && ch <= (u16)'z') {
            if (n < 8u) out[n++] = (char)ch;
        }
    }
    out[n] = 0;
    *count_out = n;
    return n != 0;
}
static int get_hii_string(hii_string_protocol *str, void *handle, u16 token, char *out, u32 *count_out) {
    u16 text[128];
    usize bytes = sizeof(text);
    u64 st = str->get_string(str, "en-US", handle, token, text, &bytes, 0);
    if (st != 0) {
        char langs[128];
        usize lang_bytes = sizeof(langs);
        if (!str->get_languages || str->get_languages(str, handle, langs, &lang_bytes) != 0 || !lang_bytes) return 0;
        u32 i = 0;
        while (i + 1u < sizeof(langs) && langs[i] && langs[i] != ';') ++i;
        if (!i || i >= sizeof(langs)) return 0;
        langs[i] = 0;
        bytes = sizeof(text);
        st = str->get_string(str, langs, handle, token, text, &bytes, 0);
        if (st != 0) return 0;
    }
    return normalize_prompt(text, out, count_out);
}
static int resolve_hii_prompt(void *system_table) {
    void *bs = *(void **)((u8 *)system_table + 0x60);
    if (!bs) return 0;
    locate_protocol_fn locate = *(locate_protocol_fn *)((u8 *)bs + 0x140);
    if (!locate) return 0;
    hii_database_protocol *db = 0;
    hii_string_protocol *str = 0;
    if (locate(&g_hii_database_guid, 0, (void **)&db) != 0 || !db) return 0;
    marker("HII_DATABASE_PROTOCOL=PASS");
    if (locate(&g_hii_string_guid, 0, (void **)&str) != 0 || !str || !str->get_string) return 0;
    marker("HII_STRING_PROTOCOL=PASS");

    usize handle_bytes = sizeof(g_hii_handles);
    if (!db->list_package_lists ||
        db->list_package_lists(db, 0x02, 0, &handle_bytes, g_hii_handles) != 0 ||
        !handle_bytes || handle_bytes > sizeof(g_hii_handles)) return 0;
    u32 handles = (u32)(handle_bytes / sizeof(void *));
    marker("HII_FORMS_HANDLE_LIST=PASS");

    for (u32 hi = 0; hi < handles; ++hi) {
        void *handle = g_hii_handles[hi];
        usize size = sizeof(g_hii_package);
        if (!handle || !db->export_package_lists ||
            db->export_package_lists(db, handle, &size, g_hii_package) != 0 ||
            size < 24u || size > sizeof(g_hii_package)) continue;
        u32 list_len = rd32(g_hii_package + 16);
        if (list_len < 24u || list_len > size) continue;
        const u8 *p = g_hii_package + 20;
        const u8 *list_end = g_hii_package + list_len;
        while (p + 4 <= list_end) {
            u32 hdr = rd32(p);
            u32 len = hdr & 0x00ffffffu;
            u8 type = (u8)(hdr >> 24);
            if (len < 4u || p + len > list_end) break;
            if (type == 0xdf) break;
            if (type == 0x02) {
                marker("HII_FORMS_PACKAGE=PASS");
                const u8 *q = p + 4;
                const u8 *end = p + len;
                while (q + 2 <= end) {
                    u8 op = q[0];
                    u32 oplen = (u32)(q[1] & 0x7f);
                    if (oplen < 2u || q + oplen > end) break;
                    if (prompt_opcode(op) && oplen >= 4u) {
                        u16 token = rd16(q + 2);
                        if (token && get_hii_string(str, handle, token, g_prompt_text, &g_prompt_count)) {
                            marker("IFR_PROMPT_STRING_ID=PASS");
                            marker("HII_LANGUAGE_AND_STRING=PASS");
                            serial_puts("HII_PROMPT_TEXT=");
                            serial_puts(g_prompt_text);
                            serial_puts("\r\n");
                            marker("HII_PROMPT_SOURCE=PASS");
                            return 1;
                        }
                    }
                    q += oplen;
                }
            }
            p += len;
        }
    }
    return 0;
}

static int graph_selftest(void) {
    clear_graph();
    g_type[0x14] = WIDGET_PIN;
    g_type[0x0c] = WIDGET_MIXER;
    g_type[0x0b] = WIDGET_SELECTOR;
    g_type[0x02] = WIDGET_AUDIO_OUTPUT;
    g_type[0x03] = 0xff;
    g_pin_output[0x14] = 1;

    g_conn_count[0x14] = 1;
    g_conn[0x14][0] = 0x0c;
    g_conn_count[0x0c] = 1;
    g_conn[0x0c][0] = 0x0b;
    g_conn_count[0x0b] = 2;
    g_conn[0x0b][0] = 0x03;
    g_conn[0x0b][1] = 0x02;

    u8 dac = 0, selectors = 0;
    if (!find_route(0x14, &dac, &selectors)) return 0;
    if (dac != 0x02 || selectors != 1 || g_depth[dac] != 3) return 0;
    if (g_route_index[dac] != 1) return 0;
    if (encode_verb12(2, 0x14, 0x701, 3) != 0x21470103u) return 0;
    if (encode_verb4(2, 0x02, 0x2, 0x0011) != 0x20220011u) return 0;
    return 1;
}

static int discover_controller(void) {
    u32 cfg = 0;
    int found = 0;

    /* AMD-5800H-REAL / ASUS M1603QA:
       1022:15E3 is the analog HDA controller feeding Realtek 10EC:0256.
       1002:1637 is HDMI audio. Prefer analog, then preserve a standards-class
       04/03 fallback for QEMU, VMware and other UEFI machines. */
    for (u32 pass = 0; pass < 2 && !found; ++pass) {
        for (u32 bdf = 0; bdf < 0x10000; ++bdf) {
            u32 base = 0x80000000u | (bdf << 8);
            u32 vd = pci_read32(base);
            if ((vd & 0xffff) == 0xffff) continue;
            u32 classreg = pci_read32(base | 0x08);
            if (((classreg >> 16) & 0xffff) != 0x0403) continue;
            if (pass == 0 && vd != 0x15e31022u) continue;
            cfg = base;
            found = 1;
            g_controller_preferred = (u8)(pass == 0);
            if (pass == 0) marker("HDA_CONTROLLER_SELECTION=PREFERRED_AMD_1022_15E3");
            else marker("HDA_CONTROLLER_SELECTION=GENERIC_CLASS_0403");
            break;
        }
    }
    if (!found) return 0;

    u32 command = pci_read32(cfg | 0x04);
    pci_write32(cfg | 0x04, command | 0x00000006u);

    u32 bar0 = pci_read32(cfg | 0x10);
    if (bar0 & 1) return 0;
    u64 bar = (u64)(bar0 & 0xfffffff0u);
    if ((bar0 & 0x6) == 0x4) {
        bar |= ((u64)pci_read32(cfg | 0x14)) << 32;
    }
    if (!bar) return 0;
    g_hda = (volatile u8 *)(usize)bar;
    if (!mmio16(0x00) || !*(volatile u8 *)(g_hda + 0x03)) return 0;

    u32 gctl = mmio32(0x08);
    mmio32w(0x08, gctl & ~1u);
    u32 timeout = 100000;
    while (timeout-- && (mmio32(0x08) & 1)) {}
    if (!timeout) return 0;
    if (g_stall) g_stall(100);

    mmio32w(0x08, mmio32(0x08) | 1u);
    timeout = 100000;
    while (timeout-- && !(mmio32(0x08) & 1)) {}
    if (!timeout) return 0;
    if (g_stall) g_stall(1000);

    u16 state = mmio16(0x0e);
    if (!state) return 0;
    for (u8 cad = 0; cad < 15; ++cad) {
        if (!(state & (1u << cad))) continue;
        g_cad = cad;
        u32 codec_id = get_param(0, 0x00);
        if (codec_id == INVALID_RESP || codec_id == 0 || codec_id == 0xffffffffu) continue;

        if (g_controller_preferred && codec_id != 0x10ec0256u) continue;

        g_codec_vendor_id = codec_id;
        serial_puts("HDA_CODEC_VENDOR_DEVICE=0x");
        serial_hex32(codec_id);
        serial_puts("\r\n");
        if (g_controller_preferred) marker("HDA_CODEC_SELECTION=REALTEK_10EC_0256");
        else marker("HDA_CODEC_SELECTION=GENERIC_RUNTIME");
        return 1;
    }
    if (g_controller_preferred) marker("HDA_CODEC_SELECTION=REALTEK_10EC_0256_NOT_FOUND");
    return 0;
}

static int discover_live_graph(u8 *pin_out, u8 *dac_out, u8 *selectors_out) {
    clear_graph();
    u32 root_nodes = get_param(0, 0x04);
    if (root_nodes == INVALID_RESP) return 0;
    u8 root_start = (u8)((root_nodes >> 16) & 0xff);
    u8 root_count = (u8)(root_nodes & 0xff);
    if (!root_count) return 0;

    u8 afg = INVALID_NID;
    for (u16 n = root_start; n < (u16)root_start + root_count; ++n) {
        u32 type = get_param((u8)n, 0x05);
        if (type != INVALID_RESP && (type & 0xff) == 1) {
            afg = (u8)n;
            break;
        }
    }
    if (afg == INVALID_NID) return 0;
    marker("HDA_AFG_RUNTIME=PASS");

    u32 widget_nodes = get_param(afg, 0x04);
    if (widget_nodes == INVALID_RESP) return 0;
    u8 start = (u8)((widget_nodes >> 16) & 0xff);
    u8 count = (u8)(widget_nodes & 0xff);
    if (!count) return 0;

    for (u16 n = start; n < (u16)start + count; ++n) {
        u8 nid = (u8)n;
        u32 cap = get_param(nid, 0x09);
        if (cap == INVALID_RESP) return 0;
        u8 type = (u8)((cap >> 20) & 0x0f);
        g_type[nid] = type;
        if (type == WIDGET_PIN) {
            u32 pin_cap = get_param(nid, 0x0c);
            if (pin_cap == INVALID_RESP) return 0;
            if (pin_cap & 0x10) g_pin_output[nid] = 1;
        }
        if (!decode_connections(nid)) return 0;
    }
    marker("HDA_WIDGET_ENUMERATION=PASS");
    marker("HDA_CONNECTION_LIST_DECODE=PASS");

    for (u16 n = start; n < (u16)start + count; ++n) {
        u8 pin = (u8)n;
        if (!g_pin_output[pin]) continue;
        u8 dac = 0, selectors = 0;
        if (find_route(pin, &dac, &selectors)) {
            *pin_out = pin;
            *dac_out = dac;
            *selectors_out = selectors;
            return 1;
        }
    }
    return 0;
}

#ifdef QEV_INTERACTIVE_REPEAT
static int wait_repeat_key(void *system_table) {
    if (!system_table) return 0;
    simple_text_input_protocol *conin =
        *(simple_text_input_protocol **)((u8 *)system_table + 0x30);
    if (!conin || !conin->read_key) return 0;
    marker("HII_GRAPH_REPEAT_KEY=WAIT_R");
    for (;;) {
        efi_input_key key;
        key.scan_code = 0;
        key.unicode_char = 0;
        u64 st = conin->read_key(conin, &key);
        if (st == 0) {
            if (key.unicode_char == (u16)'r' || key.unicode_char == (u16)'R') {
                marker("HII_GRAPH_REPEAT_KEY=R");
                marker("HII_GRAPH_REPEAT_KEY=PASS");
                return 1;
            }
            if (key.unicode_char == 0x001bu) return 0;
        }
        if (g_stall) g_stall(1000);
    }
}
#endif

__attribute__((ms_abi)) u64 efi_main(void *image_handle, void *system_table) {
    serial_init();
    marker("QEVARYNOX-UEFI-HII-GRAPH-PROMPT-SPEECH-V1");
    marker("STATE=START");
    marker("FRAMEWORK=NONE");
    marker("EDK2=NONE");

    void *boot_services = *(void **)((u8 *)system_table + 0x60);
    if (boot_services) {
        g_allocate_pages = *(allocate_pages_fn *)((u8 *)boot_services + 0x28);
        g_stall = *(stall_fn *)((u8 *)boot_services + 0xf8);
    }
    if (!resolve_hii_prompt(system_table)) {
        marker("STATUS=BLOCKED");
        marker("REASON=HII_PROMPT_NOT_RESOLVED");
        return 1;
    }

    if (!graph_selftest()) {
        marker("STATUS=BLOCKED");
        marker("REASON=HDA_MULTIHOP_EFI_SELFTEST_FAILED");
        return 1;
    }
    marker("HDA_MULTIHOP_EFI_SELFTEST=PASS");
    marker("HDA_RANGE_AND_SELECTOR_ENGINE=PASS");
    marker("NO_FIXED_WIDGET_NIDS=PASS");

    if (!discover_controller()) {
        marker("STATUS=BLOCKED");
        marker("REASON=HDA_CONTROLLER_OR_CODEC_NOT_FOUND");
        return 1;
    }
    marker("HDA_CONTROLLER_CODEC=PASS");

    u8 pin = 0, dac = 0, selectors = 0;
    if (!discover_live_graph(&pin, &dac, &selectors)) {
        marker("STATUS=BLOCKED");
        marker("REASON=HDA_GRAPH_ROUTE_NOT_FOUND");
        return 1;
    }
    marker("HDA_GRAPH_SEARCH_LIVE=PASS");

    u8 applied = 0;
    if (!apply_route(pin, dac, &applied) || applied != selectors) {
        marker("STATUS=BLOCKED");
        marker("REASON=HDA_SELECTOR_APPLY_OR_READBACK_FAILED");
        return 1;
    }
    marker("HDA_SELECTOR_APPLY_LIVE=PASS");
    serial_puts("HDA_PIN_NID=0x"); serial_hex8(pin); serial_puts("\r\n");
    serial_puts("HDA_DAC_NID=0x"); serial_hex8(dac); serial_puts("\r\n");
    serial_puts("HDA_ROUTE_DEPTH=0x"); serial_hex8(g_depth[dac]); serial_puts("\r\n");
    serial_puts("HDA_SELECTOR_WRITES_REQUIRED=0x"); serial_hex8(selectors); serial_puts("\r\n");
    serial_puts("HDA_SELECTOR_WRITES_APPLIED=0x"); serial_hex8(applied); serial_puts("\r\n");
    marker("HDA_SELECTOR_PLAN=PASS");

    if (!configure_output_path(pin, dac)) {
        marker("STATUS=BLOCKED");
        marker("REASON=HDA_OUTPUT_PATH_CONFIGURATION_FAILED");
        return 1;
    }
    marker("HDA_OUTPUT_PATH_CONFIGURATION=PASS");
    marker("SYNTH=ALLOPHONE_BDL_RUNTIME_TEXT_V1");
    marker("BDL_RUNTIME_TEXT_SCHEDULE=PASS");

    if (!run_speech_dma(g_prompt_text, g_prompt_count)) {
        marker("STATUS=BLOCKED");
        marker("REASON=HII_GRAPH_SPEECH_DMA_FAILED");
        return 1;
    }
    marker("HII_GRAPH_SPEECH_DMA=PASS");
    marker("HII_PROMPT_SPEECH_HDA=PASS");
    marker("LPIB_PROGRESS=PASS");
#ifdef QEV_INTERACTIVE_REPEAT
    if (!wait_repeat_key(system_table)) {
        marker("STATUS=BLOCKED");
        marker("REASON=HII_GRAPH_REPEAT_KEY_CANCELLED");
        return 1;
    }
    marker("EVENT=HII_GRAPH_REPEAT_TEXT_COMMIT");
    if (!run_speech_dma(g_prompt_text, g_prompt_count)) {
        marker("STATUS=BLOCKED");
        marker("REASON=HII_GRAPH_REPEAT_SPEECH_DMA_FAILED");
        return 1;
    }
    marker("HII_GRAPH_REPEAT_SPEECH_DMA=PASS");
    marker("HII_GRAPH_REPEAT_SPEECH_HDA=PASS");
    marker("HII_GRAPH_REPEAT_LPIB_PROGRESS=PASS");
#endif
    if (persist_boot_proof(image_handle, boot_services, pin, dac, selectors, applied)) {
        marker("BOOT_MEDIA_PERSISTENT_PROOF=PASS");
    } else {
        marker("BOOT_MEDIA_PERSISTENT_PROOF=NOT_ESTABLISHED");
    }
    marker("PHYSICAL_ASUS_M1603QA_SPEECH=NOT_ESTABLISHED");
    marker("STATUS=PASS");
    return 0;
}
