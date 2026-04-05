fn patch() {
    #{
        id: "change_bms_encryption_keys",
        name: "Change BMS Encryption Keys",
        description: "Lets the user change the BMS encryption key and nonce to, for example, use an XRC BMS with a GT controller.",
        boards: #{
            "GT": [
                #{ versions: [6109], targets: [
                    #{ offset: 0x12009, size: 32, blind: true },
                ]},
                #{ versions: [6210], targets: [
                    #{ offset: 0x312, size: 32, blind: true },
                ]},
            ],
            "GTS": [
                #{ versions: [6167], targets: [
                    #{ offset: 0x2F8, size: 32, blind: true },
                ]},
                #{ versions: [6208], targets: [
                    #{ offset: 0x30B, size: 32, blind: true },
                ]},
            ],
            "XRC": [
                #{ versions: [6213], targets: [
                    #{ offset: 0x314, size: 32, blind: true },
                ]},
            ],
        }
    }
}

fn parameters() {
    [#{
         name: "enc_key",
         label: "Key",
         description: "Encryption Key as hex.",
         kind: "hex",
         len: 16,
    }, #{
         name: "enc_nonce",
         label: "Nonce",
         description: "Encryption Nonce as hex.",
         kind: "hex",
         len: 16,
    }]
}

fn apply(params) {
    [#{
        offset: TARGETS[0].offset,
        bytes: params.enc_key + params.enc_nonce,
    }]
}

fn read(fw) {
    let data = fw[`${TARGETS[0].offset}`];
    #{
        enc_key: data.extract(0, 16),
        enc_nonce: data.extract(16, 16),
    }
}
