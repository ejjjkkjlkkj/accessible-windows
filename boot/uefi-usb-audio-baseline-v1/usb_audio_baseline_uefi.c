typedef unsigned char u8;
typedef unsigned short u16;
typedef unsigned int u32;
typedef unsigned long long u64;
typedef unsigned long long usize;

typedef struct {
    u32 data1;
    u16 data2;
    u16 data3;
    u8 data4[8];
} efi_guid;

typedef struct __attribute__((packed)) {
    u8 length;
    u8 descriptor_type;
    u16 bcd_usb;
    u8 device_class;
    u8 device_subclass;
    u8 device_protocol;
    u8 max_packet_size0;
    u16 vendor_id;
    u16 product_id;
    u16 bcd_device;
    u8 manufacturer;
    u8 product;
    u8 serial_number;
    u8 num_configurations;
} usb_device_descriptor;

typedef struct __attribute__((packed)) {
    u8 length;
    u8 descriptor_type;
    u8 interface_number;
    u8 alternate_setting;
    u8 num_endpoints;
    u8 interface_class;
    u8 interface_subclass;
    u8 interface_protocol;
    u8 interface_string;
} usb_interface_descriptor;

typedef struct __attribute__((packed)) {
    u8 length;
    u8 descriptor_type;
    u8 endpoint_address;
    u8 attributes;
    u16 max_packet_size;
    u8 interval;
} usb_endpoint_descriptor;

typedef struct __attribute__((packed)) {
    u8 request_type;
    u8 request;
    u16 value;
    u16 index;
    u16 length;
} usb_device_request;

struct usb_io_protocol;
typedef u64 (*usb_control_transfer_fn)(struct usb_io_protocol *, usb_device_request *, u32, u32, void *, usize, u32 *);
typedef u64 (*usb_isochronous_transfer_fn)(struct usb_io_protocol *, u8, void *, usize, u32 *);
typedef u64 (*usb_get_device_descriptor_fn)(struct usb_io_protocol *, usb_device_descriptor *);
typedef u64 (*usb_get_interface_descriptor_fn)(struct usb_io_protocol *, usb_interface_descriptor *);
typedef u64 (*usb_get_endpoint_descriptor_fn)(struct usb_io_protocol *, u8, usb_endpoint_descriptor *);

typedef struct usb_io_protocol {
    usb_control_transfer_fn control_transfer;
    void *bulk_transfer;
    void *async_interrupt_transfer;
    void *sync_interrupt_transfer;
    usb_isochronous_transfer_fn isochronous_transfer;
    void *async_isochronous_transfer;
    usb_get_device_descriptor_fn get_device_descriptor;
    void *get_config_descriptor;
    usb_get_interface_descriptor_fn get_interface_descriptor;
    usb_get_endpoint_descriptor_fn get_endpoint_descriptor;
    void *get_string_descriptor;
    void *get_supported_languages;
    void *port_reset;
} usb_io_protocol;

typedef u64 (*locate_handle_buffer_fn)(u32, const void *, void *, usize *, void ***);
typedef u64 (*handle_protocol_fn)(void *, const void *, void **);
typedef u64 (*free_pool_fn)(void *);
typedef u64 (*stall_fn)(usize);

static const efi_guid g_usb_io_guid =
    {0x2b2f68d6u,0x0cd2u,0x44cfu,{0x8e,0x8b,0xbb,0xa2,0x0b,0x1b,0x5b,0x75}};

#define EFI_SUCCESS 0ull
#define EFI_UNSUPPORTED 0x8000000000000003ull
#define BY_PROTOCOL 2u
#define USB_CLASS_AUDIO 0x01u
#define USB_SUBCLASS_AUDIO_STREAMING 0x02u
#define USB_REQ_SET_INTERFACE 0x0bu
#define USB_REQUEST_TYPE_STD_INTERFACE_OUT 0x01u
#define EFI_USB_NO_DATA 2u
#define QEMU_USB_AUDIO_VENDOR 0x46f4u
#define QEMU_USB_AUDIO_PRODUCT 0x0002u
#define AUDIO_ENDPOINT_OUT 0x01u
#define AUDIO_PACKET_BYTES 192u

static inline void outb(u16 port, u8 value) {
    __asm__ volatile("outb %0, %1" :: "a"(value), "d"(port));
}
static inline u8 inb(u16 port) {
    u8 value;
    __asm__ volatile("inb %1, %0" : "=a"(value) : "d"(port));
    return value;
}
static void serial_init(void) {
    outb(0x3f9, 0x00); outb(0x3fb, 0x80); outb(0x3f8, 0x03);
    outb(0x3f9, 0x00); outb(0x3fb, 0x03); outb(0x3fa, 0xc7); outb(0x3fc, 0x0b);
}
static void serial_char(char ch) {
    u32 timeout = 1000000u;
    while (timeout-- && !(inb(0x3fd) & 0x20u)) {}
    outb(0x3f8, (u8)ch);
}
static void serial_puts(const char *s) { while (*s) serial_char(*s++); }
static void serial_hex8(u8 value) {
    static const char h[] = "0123456789ABCDEF";
    serial_char(h[(value >> 4) & 0xf]); serial_char(h[value & 0xf]);
}
static void serial_hex16(u16 value) { serial_hex8((u8)(value >> 8)); serial_hex8((u8)value); }
static void serial_hex64(u64 value) {
    for (int shift = 56; shift >= 0; shift -= 8) serial_hex8((u8)(value >> shift));
}
static void marker(const char *s) { serial_puts(s); serial_puts("\r\n"); }
static void status_line(const char *key, u64 st) {
    serial_puts(key); serial_puts("=0x"); serial_hex64(st); serial_puts("\r\n");
}

static void make_tone_packet(u8 *packet) {
    for (u32 frame = 0; frame < 48u; ++frame) {
        short sample = (frame < 24u) ? 9000 : -9000;
        u32 off = frame * 4u;
        packet[off + 0u] = (u8)(sample & 0xff);
        packet[off + 1u] = (u8)((sample >> 8) & 0xff);
        packet[off + 2u] = packet[off + 0u];
        packet[off + 3u] = packet[off + 1u];
    }
}

__attribute__((ms_abi)) u64 efi_main(void *image_handle, void *system_table) {
    (void)image_handle;
    serial_init();
    marker("QEVARYNOX-UEFI-USB-AUDIO-BASELINE-V1");
    marker("STATE=START");
    marker("PURPOSE=PROVE_FIRMWARE_USB_ISOCHRONOUS_BASELINE");
    marker("PCM=48000HZ_S16LE_STEREO_192B_PER_1MS_FRAME");

    if (!system_table) { marker("STATUS=BLOCKED"); marker("REASON=NO_SYSTEM_TABLE"); return 1; }
    void *bs = *(void **)((u8 *)system_table + 0x60);
    if (!bs) { marker("STATUS=BLOCKED"); marker("REASON=NO_BOOT_SERVICES"); return 1; }

    locate_handle_buffer_fn locate_handles = *(locate_handle_buffer_fn *)((u8 *)bs + 0x138);
    handle_protocol_fn handle_protocol = *(handle_protocol_fn *)((u8 *)bs + 0x98);
    free_pool_fn free_pool = *(free_pool_fn *)((u8 *)bs + 0x48);
    stall_fn stall = *(stall_fn *)((u8 *)bs + 0xf8);
    if (!locate_handles || !handle_protocol || !free_pool) {
        marker("STATUS=BLOCKED"); marker("REASON=BOOT_SERVICE_POINTER_MISSING"); return 1;
    }

    usize count = 0;
    void **handles = 0;
    u64 st = locate_handles(BY_PROTOCOL, &g_usb_io_guid, 0, &count, &handles);
    status_line("USB_LOCATE_HANDLE_BUFFER_STATUS", st);
    if (st != EFI_SUCCESS || !handles || !count) {
        marker("STATUS=BLOCKED"); marker("REASON=USB_IO_HANDLE_NOT_FOUND"); return 1;
    }
    marker("USB_IO_HANDLE_LIST=PASS");

    int saw_audio = 0;
    int saw_qemu_audio = 0;
    int attempted_iso = 0;
    int iso_success = 0;
    int iso_unsupported = 0;

    for (usize i = 0; i < count; ++i) {
        usb_io_protocol *usb = 0;
        st = handle_protocol(handles[i], &g_usb_io_guid, (void **)&usb);
        if (st != EFI_SUCCESS || !usb || !usb->get_device_descriptor ||
            !usb->get_interface_descriptor || !usb->isochronous_transfer ||
            !usb->control_transfer) continue;

        usb_device_descriptor dev;
        usb_interface_descriptor iface;
        if (usb->get_device_descriptor(usb, &dev) != EFI_SUCCESS) continue;
        if (usb->get_interface_descriptor(usb, &iface) != EFI_SUCCESS) continue;

        serial_puts("USB_INTERFACE VID=0x"); serial_hex16(dev.vendor_id);
        serial_puts(" PID=0x"); serial_hex16(dev.product_id);
        serial_puts(" IF=0x"); serial_hex8(iface.interface_number);
        serial_puts(" ALT=0x"); serial_hex8(iface.alternate_setting);
        serial_puts(" CLASS=0x"); serial_hex8(iface.interface_class);
        serial_puts(" SUBCLASS=0x"); serial_hex8(iface.interface_subclass);
        serial_puts(" EPS=0x"); serial_hex8(iface.num_endpoints); serial_puts("\r\n");

        if (iface.interface_class != USB_CLASS_AUDIO) continue;
        saw_audio = 1;
        marker("USB_AUDIO_INTERFACE=PASS");
        if (dev.vendor_id == QEMU_USB_AUDIO_VENDOR && dev.product_id == QEMU_USB_AUDIO_PRODUCT) {
            saw_qemu_audio = 1;
            marker("QEMU_USB_AUDIO_46F4_0002=PASS");
        }
        if (iface.interface_subclass != USB_SUBCLASS_AUDIO_STREAMING) continue;

        usb_device_request req;
        req.request_type = USB_REQUEST_TYPE_STD_INTERFACE_OUT;
        req.request = USB_REQ_SET_INTERFACE;
        req.value = 1u;
        req.index = iface.interface_number;
        req.length = 0u;
        u32 transfer_result = 0xffffffffu;
        st = usb->control_transfer(usb, &req, EFI_USB_NO_DATA, 1000u, 0, 0u, &transfer_result);
        status_line("USB_AUDIO_SET_INTERFACE_STATUS", st);
        serial_puts("USB_AUDIO_SET_INTERFACE_RESULT=0x"); serial_hex64((u64)transfer_result); serial_puts("\r\n");
        if (st != EFI_SUCCESS) continue;
        marker("USB_AUDIO_ALT1=PASS");

        usb_endpoint_descriptor ep;
        if (usb->get_endpoint_descriptor && usb->get_endpoint_descriptor(usb, 0u, &ep) == EFI_SUCCESS) {
            serial_puts("USB_AUDIO_ENDPOINT=0x"); serial_hex8(ep.endpoint_address);
            serial_puts(" ATTR=0x"); serial_hex8(ep.attributes);
            serial_puts(" MPS=0x"); serial_hex16(ep.max_packet_size);
            serial_puts(" INTERVAL=0x"); serial_hex8(ep.interval); serial_puts("\r\n");
        } else {
            marker("USB_AUDIO_ENDPOINT_DESCRIPTOR=NOT_EXPOSED_AFTER_ALT_SWITCH");
        }

        u8 packet[AUDIO_PACKET_BYTES];
        make_tone_packet(packet);
        transfer_result = 0xffffffffu;
        attempted_iso = 1;
        st = usb->isochronous_transfer(usb, AUDIO_ENDPOINT_OUT, packet, AUDIO_PACKET_BYTES, &transfer_result);
        status_line("USB_AUDIO_ISOCHRONOUS_STATUS", st);
        serial_puts("USB_AUDIO_ISOCHRONOUS_RESULT=0x"); serial_hex64((u64)transfer_result); serial_puts("\r\n");
        if (st == EFI_SUCCESS) {
            iso_success = 1;
            marker("USB_AUDIO_ISOCHRONOUS=PASS");
            marker("USB_AUDIO_FIRMWARE_PATH=AVAILABLE");
            if (stall) stall(1000u);
        } else if (st == EFI_UNSUPPORTED) {
            iso_unsupported = 1;
            marker("USB_AUDIO_ISOCHRONOUS=EFI_UNSUPPORTED");
            marker("USB_AUDIO_CUSTOM_XHCI_REQUIRED=PASS");
        } else {
            marker("USB_AUDIO_ISOCHRONOUS=FAILED_OTHER");
        }
        break;
    }

    free_pool(handles);
    if (!saw_audio) { marker("STATUS=BLOCKED"); marker("REASON=USB_AUDIO_INTERFACE_NOT_FOUND"); return 1; }
    if (!attempted_iso) { marker("STATUS=BLOCKED"); marker("REASON=USB_AUDIO_STREAMING_INTERFACE_NOT_FOUND"); return 1; }
    if (saw_qemu_audio) marker("QEMU_USB_AUDIO_TARGET=CONFIRMED");
    if (iso_success) { marker("STATUS=PASS"); return 0; }
    if (iso_unsupported) { marker("STATUS=EXPECTED_BASELINE_BLOCK"); return 0; }
    marker("STATUS=BLOCKED"); marker("REASON=ISOCHRONOUS_FAILED_NOT_UNSUPPORTED"); return 1;
}
