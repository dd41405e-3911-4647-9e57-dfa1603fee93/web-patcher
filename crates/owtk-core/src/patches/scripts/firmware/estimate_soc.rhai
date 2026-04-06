fn patch() {
    #{
        id: "estimate_soc",
        name: "Estimate SoC",
        description: "Estimates the BMS state-of-charge with voltage-based estimation matched to the installed cell chemistry.",
        experimental: true,
        sram: #{ bss: 8 },
        boards: #{
            "XR": [
                #{ versions: [4142], targets: [
                    #{ offset: 0xB91A, original: "DF F8 28 16 08 70 65 28 A8 BF 64 20 01 DA 08 B9 01 20 08 70", meta: #{ kind: "soc_caller", branch_to: 0xB92E } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20001AA4, soc_percentage_addr: 0x20001C36, amp_drain_addr: 0x20001BCC, amp_regen_addr: 0x20001BD4} },
                ]},
                // Not enough free flash for the shellcode
                //#{ versions: [4165], targets: [
                //    #{ offset: 0xBB66, original: "30 79 DF F8 8C 17 08 70 65 28 A8 BF 64 20 01 DA 08 B9 01 20 08 70", meta: #{ kind: "soc_caller", branch_to: 0xBB7C } },
                //    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20001B50, soc_percentage_addr: 0x20001D06, amp_drain_addr: 0x20001C80, amp_regen_addr: 0x20001C88 } },
                //]},
            ],
            "Pint": [
                #{ versions: [5040], targets: [
                    #{ offset: 0xBA24, original: "DF F8 50 16 08 70 65 28 A8 BF 64 20 01 DA 08 B9 01 20 08 70", meta: #{ kind: "soc_caller", branch_to: 0xBA38 } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20001ABC, soc_percentage_addr: 0x20001C38, amp_drain_addr: 0x20001BD4, amp_regen_addr: 0x20001BD8 } },
                ]},
                #{ versions: [5100], targets: [
                    #{ offset: 0xB6A8, original: "30 79 DF F8 E0 17 08 70 65 28 A8 BF 64 20 01 DA 08 B9 01 20 08 70", meta: #{ kind: "soc_caller", branch_to: 0xB6BE } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20001F7C, soc_percentage_addr: 0x20002116, amp_drain_addr: 0x20002088, amp_regen_addr: 0x20002090 } },
                ]},
                #{ versions: [5205], targets: [
                    #{ offset: 0xB0DC, original: "30 79 DF F8 DC 17 08 70 65 28 A8 BF 64 20 01 DA 08 B9 01 20 08 70", meta: #{ kind: "soc_caller", branch_to: 0xB0F2 } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20001FAC, soc_percentage_addr: 0x2000216B, amp_drain_addr: 0x200020CC, amp_regen_addr: 0x200020D4 } },
                ]},
                #{ versions: [5207], targets: [
                    #{ offset: 0xB0EA, original: "30 79 DF F8 DC 17 08 70 65 28 A8 BF 64 20 01 DA 08 B9 01 20 08 70", meta: #{ kind: "soc_caller", branch_to: 0xB100 } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20001FAC, soc_percentage_addr: 0x2000216B, amp_drain_addr: 0x200020CC, amp_regen_addr: 0x200020D4 } },
                ]},
            ],
            "PintX": [
                #{ versions: [5076], targets: [
                    #{ offset: 0xB8B2, original: "30 79 DF F8 84 17 08 70 65 28 A8 BF 64 20 01 DA 08 B9 01 20 08 70", meta: #{ kind: "soc_caller", branch_to: 0xB8C8 } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20001B38, soc_percentage_addr: 0x20001CDB, amp_drain_addr: 0x20001C58, amp_regen_addr: 0x20001C60 } },
                ]},
                #{ versions: [5100], targets: [
                    #{ offset: 0xB6A8, original: "30 79 DF F8 E0 17 08 70 65 28 A8 BF 64 20 01 DA 08 B9 01 20 08 70", meta: #{ kind: "soc_caller", branch_to: 0xB6BE } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20001F7C, soc_percentage_addr: 0x20002116, amp_drain_addr: 0x20002088, amp_regen_addr: 0x20002090 } },
                ]},
                #{ versions: [5200], targets: [
                    #{ offset: 0xB340, original: "30 79 DF F8 DC 17 08 70 65 28 A8 BF 64 20 01 DA 08 B9 01 20 08 70", meta: #{ kind: "soc_caller", branch_to: 0xB356 } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20001FC4, soc_percentage_addr: 0x20002183, amp_drain_addr: 0x200020E4, amp_regen_addr: 0x200020EC } },
                ]},
            ],
            "GT": [
                #{ versions: [6109], targets: [
                    #{ offset: 0x10F46, original: "60 78 DF F8 90 1B 08 70 65 28 A8 BF 64 20 80 F2 EE 80 00 28 40 F0 EC 80 01 20 E8 E0", meta: #{ kind: "soc_caller", branch_to: 0x11136 } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20002988, soc_percentage_addr: 0x20002B80, amp_drain_addr: 0x20002B04, amp_regen_addr: 0x20002B0C } },
                ]},
                #{ versions: [6210], targets: [
                    #{ offset: 0x14B1A, original: "60 78 DF F8 90 1B 08 70 65 28 A8 BF 64 20 80 F2 EE 80 00 28 40 F0 EC 80 01 20 E8 E0", meta: #{ kind: "soc_caller", branch_to: 0x14D0A } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20002AF8, soc_percentage_addr: 0x20002EFC, amp_drain_addr: 0x20002E74, amp_regen_addr: 0x20002E7C } },
                ]},
            ],
            "GTS": [
                #{ versions: [6167], targets: [
                    #{ offset: 0x13390, original: "60 78 DF F8 74 1B 08 70 65 28 A8 BF 64 20 80 F2 EE 80 00 28 40 F0 EC 80 01 20 E8 E0", meta: #{ kind: "soc_caller", branch_to: 0x13580 } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20002AA4, soc_percentage_addr: 0x20002E78, amp_drain_addr: 0x20002DEC, amp_regen_addr: 0x20002DF4 } },
                ]},
                #{ versions: [6208], targets: [
                    #{ offset: 0x13FB4, original: "60 78 DF F8 7C 1B 08 70 65 28 A8 BF 64 20 80 F2 EE 80 00 28 40 F0 EC 80 01 20 E8 E0", meta: #{ kind: "soc_caller", branch_to: 0x141A4 } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20002AD4, soc_percentage_addr: 0x20002EEC, amp_drain_addr: 0x20002E64, amp_regen_addr: 0x20002E6C } },
                ]},
            ],
            "XRC": [
                #{ versions: [6213], targets: [
                    #{ offset: 0x14986, original: "60 78 DF F8 8C 1B 08 70 65 28 A8 BF 64 20 80 F2 EE 80 00 28 40 F0 EC 80 01 20 E8 E0", meta: #{ kind: "soc_caller", branch_to: 0x14B76 } },
                    #{ append: true, size: 274, meta: #{ kind: "soc_estimator", cell_voltages_addr: 0x20002B04, soc_percentage_addr: 0x20002EFC, amp_drain_addr: 0x20002E74, amp_regen_addr: 0x20002E7C } },
                ]},
            ],
        }
    }
}

fn parameters() {
    [#{
        name: "chemistry",
        label: "Cell Chemistry",
        description: "Discharge curve for the replacement cells.",
        kind: "enum",
        options: [
            #{ value: "P42A", label: "Molicel P42A" },
            #{ value: "P45B", label: "Molicel P45B" },
            #{ value: "P50B", label: "Molicel P50B" },
            #{ value: "P28A", label: "Molicel P28A" },
            #{ value: "DG40", label: "Lishen DG40" },
            #{ value: "VTC6", label: "Sony VTC6" },
            #{ value: "S50S", label: "Samsung 50S" },
            #{ value: "RS50", label: "Samsung RS50" },
        ],
        initial: "P42A",
    }, #{
        name: "capacity_mah",
        label: "Pack Capacity (mAh)",
        description: "Total capacity of the replacement pack in milliamp-hours.",
        kind: "integer",
        min: 2000,
        max: 20000,
        initial: 4000,
    }]
}

// Discharge curves: 11 voltage points from 100% to 0% in 10% steps.
// Values in millivolts.
fn voltage_table(chemistry) {
    let curves = #{
        "P42A": [4140, 4050, 3910, 3830, 3740, 3650, 3570, 3480, 3380, 3000, 2800],
        "P45B": [4140, 4080, 4000, 3910, 3820, 3730, 3640, 3560, 3450, 3230, 2900],
        "P50B": [4150, 4047, 3960, 3880, 3790, 3700, 3595, 3466, 3290, 3030, 2850],
        "P28A": [4140, 4090, 3980, 3880, 3770, 3690, 3630, 3550, 3450, 3240, 2900],
        "DG40": [4150, 4020, 3910, 3830, 3750, 3610, 3490, 3350, 3170, 2810, 2700],
        "VTC6": [4140, 4000, 3900, 3800, 3700, 3600, 3500, 3400, 3300, 3100, 2800],
        "S50S": [4150, 4040, 3900, 3820, 3740, 3640, 3520, 3380, 3160, 3000, 2900],
        "RS50": [4160, 4055, 4002, 3888, 3790, 3672, 3532, 3388, 3198, 2989, 2700],
    };
    curves[chemistry]
}

fn apply(params) {
    let chemistry = params["chemistry"];
    let capacity_mah = params["capacity_mah"];
    let curve = voltage_table(chemistry);

    // Find the SOC estimator append target to get its allocated offset.
    let estimator_offset = 0;
    for t in TARGETS { if t.meta["kind"] == "soc_estimator" { estimator_offset = t.offset; } }

    // SOC estimator — compiled from soc_estimator.c with arm-none-eabi-gcc.
    // Target: cortex-m3 (compatible with both M3/F1 and M4/F4 boards).
    // Linked at 0x08060000; single literal-pool entry at +0xDC holds the
    // link-time address of the config struct (0x080600E0 = code+0xE0).
    // The patcher relocates that entry to the actual runtime address.
    // 224 bytes code + 28 bytes config = 252 bytes.
    // Voltage table (22 bytes) appended by patcher = 274 total.
    //
    // Config struct at code+0xE0:
    //   +0xE0  u32  cell_voltages_addr
    //   +0xE4  u32  soc_percentage_addr
    //   +0xE8  u32  amp_drain_addr
    //   +0xEC  u32  amp_regen_addr
    //   +0xF0  u32  bss_addr
    //   +0xF4  u32  table_addr
    //   +0xF8  u16  cell_count
    //   +0xFA  u16  capacity_mah
    let soc_code = hex_to_blob("2de9f043354b0021d3f8009058689e68dc681a69d3f814c0b3f818805d8b4ff6ff7341453bdb4ff6ff718b4235d000213cf811e04f009e453fd8002942d04ff00a08a3eb0e0308fb03f3674437f8027ca8eb0101a7eb0e0793fbf7f308fb0133642ba8bf642336682168761a1179a5292ad1c82714685179341b94fbf7f494fbf5f4091b5c1a0234072c1dd80129b8bf01216429a8bf64210170bde8f08339f811709e46bfb2bb4228bfbe461ffa8efe002f18bf73460131b3e701310b29b7d10023cde76423cee7a521012b11715371b8bf012316600370dfe700bfe00006080100adde0200adde0300adde0400adde0500adde0600adde01be02be");
    let LINK_BASE = 0x08060000;
    let CFG_OFF = 0xE0;  // config struct offset in binary
    let POOL_OFF = 0xDC; // literal pool entry for &cfg

    // Board-gen constants.
    let cell_count = if BOARD_GEN == "GTS" { 27 } else if BOARD_GEN == "GT" || BOARD_GEN == "XRC" { 18 } else { 15 };
    let fw_base = if BOARD_GEN == "GT" || BOARD_GEN == "GTS" || BOARD_GEN == "XRC" { 0x08020000 } else { 0x08003000 };
    let bss_addr = SRAM["bss"];
    let estimator_addr = fw_base + estimator_offset;

    // Voltage table: 11 x u16 LE = 22 bytes.
    // F1: raw = millivolts (scale x1). F4: raw = 0.1 mV (scale x10).
    let is_f4 = BOARD_GEN == "GT" || BOARD_GEN == "GTS" || BOARD_GEN == "XRC";
    let voltage_scale = if is_f4 { 10 } else { 1 };
    let table_blob = blob();
    for v in curve {
        table_blob += encode_u16le(v * voltage_scale);
    }

    TARGETS.map(|t| {
        let kind = t.meta["kind"];
        if kind == "soc_caller" {
            let bl = thumb_bl(t.offset, estimator_offset);
            let b = thumb_b(t.offset + 4, t.meta["branch_to"]);
            #{ offset: t.offset, bytes: bl + b + nop_sled(t.len - 6) }
        } else {
            // soc_estimator: relocate literal pool and patch config.
            let code = soc_code;
            let m = t.meta;

            // Relocate the single literal-pool entry from link-time to runtime.
            code = blob_write(code, POOL_OFF, encode_u32le(estimator_addr + CFG_OFF));

            // Patch config struct at code+0xE0.
            code = blob_write(code, CFG_OFF + 0x00, encode_u32le(m["cell_voltages_addr"]));
            code = blob_write(code, CFG_OFF + 0x04, encode_u32le(m["soc_percentage_addr"]));
            code = blob_write(code, CFG_OFF + 0x08, encode_u32le(m["amp_drain_addr"]));
            code = blob_write(code, CFG_OFF + 0x0C, encode_u32le(m["amp_regen_addr"]));
            code = blob_write(code, CFG_OFF + 0x10, encode_u32le(bss_addr));
            code = blob_write(code, CFG_OFF + 0x14, encode_u32le(estimator_addr + 252));  // table follows code+config
            code = blob_write(code, CFG_OFF + 0x18, encode_u16le(cell_count));
            code = blob_write(code, CFG_OFF + 0x1A, encode_u16le(capacity_mah));

            #{ offset: t.offset, bytes: code + table_blob }
        }
    })
}
