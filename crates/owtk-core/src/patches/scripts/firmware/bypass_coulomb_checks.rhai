fn patch() {
    #{
        id: "bypass_coulomb_checks",
        name: "Bypass Coulomb Checks",
        description: "Allows the use of bigger third-party battery packs that would normally trigger an Ah compare error.",
        boards: #{
            "XR": [
                #{ versions: [4165], targets: [
                    #{ offset: 0xBF8C, original: "63 DB", meta: #{ branch_to: 0xC056 } },
                    #{ offset: 0x6598, original: "0C D3", meta: #{ branch_to: 0x65B4 } },
                ]},
            ],
            "Pint": [
                #{ versions: [5100], targets: [
                    #{ offset: 0xBAD0, original: "64 DB", meta: #{ branch_to: 0xBB9C } },
                    #{ offset: 0x366, original: "0C D3", meta: #{ branch_to: 0x382 } },
                ]},
                #{ versions: [5205], targets: [
                    #{ offset: 0xB500, original: "66 DB", meta: #{ branch_to: 0xB5D0 } },
                    #{ offset: 0x376, original: "0C D3", meta: #{ branch_to: 0x392 } },
                ]},
                #{ versions: [5207], targets: [
                    #{ offset: 0xB510, original: "66 DB", meta: #{ branch_to: 0xB5E0 } },
                    #{ offset: 0x376, original: "0C D3", meta: #{ branch_to: 0x392 } },
                ]},
            ],
            "PintX": [
                #{ versions: [5076], targets: [
                    #{ offset: 0xBCDA, original: "63 DB", meta: #{ branch_to: 0xBDA4 } },
                    #{ offset: 0x657E, original: "0C D3", meta: #{ branch_to: 0x659A } },
                ]},
                #{ versions: [5100], targets: [
                    #{ offset: 0xBAD0, original: "64 DB", meta: #{ branch_to: 0xBB9C } },
                    #{ offset: 0x366, original: "0C D3", meta: #{ branch_to: 0x382 } },
                ]},
                #{ versions: [5200], targets: [
                    #{ offset: 0xB764, original: "66 DB", meta: #{ branch_to: 0xB834 } },
                    #{ offset: 0x376, original: "0C D3", meta: #{ branch_to: 0x392 } },
                ]},
            ],
            "GT": [
                #{ versions: [6109], targets: [
                    #{ offset: 0x1144A, original: "C0 F2 75 81", meta: #{ branch_to: 0x11738 } },
                    #{ offset: 0xC09C, original: "10 D3", meta: #{ branch_to: 0xC0C0 } },
                ]},
                #{ versions: [6210], targets: [
                    #{ offset: 0x1501E, original: "C0 F2 8E 81", meta: #{ branch_to: 0x1533E } },
                    #{ offset: 0xEE9E, original: "0F D3", meta: #{ branch_to: 0xEEC0 } },
                ]},
            ],
            "GTS": [
                #{ versions: [6167], targets: [
                    #{ offset: 0x13880, original: "C0 F2 8E 81", meta: #{ branch_to: 0x13BA0 } },
                    #{ offset: 0xDFDA, original: "0E D3", meta: #{ branch_to: 0xDFFA } },
                ]},
                #{ versions: [6208], targets: [
                    #{ offset: 0x144A4, original: "C0 F2 8E 81", meta: #{ branch_to: 0x147C4 } },
                    #{ offset: 0xE2F2, original: "0E D3", meta: #{ branch_to: 0xE312 } },
                ]},
            ],
            "XRC": [
                #{ versions: [6213], targets: [
                    #{ offset: 0x14E84, original: "C0 F2 8E 81", meta: #{ branch_to: 0x151A4 } },
                    #{ offset: 0xEEC6, original: "0F D3", meta: #{ branch_to: 0xEEE8 } },
                ]},
            ],
        }
    }
}

fn parameters() {
    []
}

fn apply(params) {
    TARGETS.map(|t| {
        let branch = if t.len == 4 { thumb_b_w(t.offset, t.meta["branch_to"]) } else { thumb_b(t.offset, t.meta["branch_to"]) };
        #{ offset: t.offset, bytes: branch }
    })
}
