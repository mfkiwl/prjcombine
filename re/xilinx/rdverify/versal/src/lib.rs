use prjcombine_re_xilinx_naming_versal::ExpandedNamedDevice;
use prjcombine_re_xilinx_rawdump::Part;
use prjcombine_re_xilinx_rdverify::{BelContext, SitePinDir, Verifier, verify};
use prjcombine_versal::{expanded::UbumpId, grid::DisabledPart};
use unnamed_entity::EntityId;

fn verify_slice(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let kind = if bel.bel.pins.contains_key("WE") {
        "SLICEM"
    } else {
        "SLICEL"
    };
    let mut pins = vec![("CIN", SitePinDir::In), ("COUT", SitePinDir::Out)];
    if kind == "SLICEM" {
        pins.extend([("SRL_IN_B", SitePinDir::In), ("SRL_OUT_B", SitePinDir::Out)]);
    }
    vrf.verify_bel(bel, kind, &pins, &[]);
    for (pin, dir) in pins {
        vrf.claim_node(&[bel.fwire(pin)]);
        if dir == SitePinDir::In {
            vrf.claim_pip(bel.crd(), bel.wire(pin), bel.wire_far(pin));
        } else {
            vrf.claim_pip(bel.crd(), bel.wire_far(pin), bel.wire(pin));
        }
    }
    vrf.claim_node(&[bel.fwire_far("COUT")]);
    if let Some(obel) = vrf.find_bel_delta(bel, 0, -1, bel.key) {
        vrf.verify_node(&[bel.fwire_far("CIN"), obel.fwire_far("COUT")]);
    } else {
        vrf.claim_node(&[bel.fwire_far("CIN")]);
    }
    if kind == "SLICEM" {
        vrf.claim_node(&[bel.fwire_far("SRL_OUT_B")]);
        if bel.row.to_idx() % 48 != 0 {
            if let Some(obel) = vrf.find_bel_delta(bel, 0, -1, bel.key) {
                vrf.verify_node(&[bel.fwire_far("SRL_IN_B"), obel.fwire_far("SRL_OUT_B")]);
            } else {
                vrf.claim_node(&[bel.fwire_far("SRL_IN_B")]);
            }
        } else {
            vrf.claim_node(&[bel.fwire_far("SRL_IN_B")]);
        }
    }
}

fn verify_laguna(endev: &ExpandedNamedDevice, vrf: &mut Verifier, bel: &BelContext<'_>) {
    let edev = endev.edev;
    for i in 0..6 {
        vrf.claim_pip(
            bel.crd(),
            bel.wire(&format!("OUT{i}")),
            bel.wire(&format!("UBUMP{i}")),
        );
        vrf.claim_pip(
            bel.crd(),
            bel.wire(&format!("UBUMP{i}")),
            bel.wire(&format!("IN{i}")),
        );
        let bump = UbumpId::from_idx(i);
        if let Some(conns) = edev.sll.get(&(bel.die, bel.col + 1, bel.row)) {
            if !conns.cursed[bump] {
                if let Some((odie, ocol, orow, obump)) = conns.conns[bump] {
                    let obel = vrf.find_bel(odie, (ocol - 1, orow), "LAGUNA").unwrap();
                    if (bel.die, bel.col + 1, bel.row, bump) < (odie, ocol, orow, obump) {
                        vrf.claim_node(&[
                            bel.fwire(&format!("UBUMP{i}")),
                            obel.fwire(&format!("UBUMP{obump}")),
                        ]);
                    } else {
                        vrf.verify_node(&[
                            bel.fwire(&format!("UBUMP{i}")),
                            obel.fwire(&format!("UBUMP{obump}")),
                        ]);
                    }
                } else {
                    vrf.claim_node(&[bel.fwire(&format!("UBUMP{i}"))]);
                }
            }
        } else {
            vrf.claim_node(&[bel.fwire(&format!("UBUMP{i}"))]);
        }
    }
}

fn verify_dsp(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let mut inps = vec![];
    let mut outps = vec![];
    let mut cascs = vec![];
    let obel_cplx = vrf.find_bel_sibling(bel, "DSP_CPLX");
    let obel_odsp = vrf.find_bel_sibling(bel, if bel.key == "DSP1" { "DSP0" } else { "DSP1" });
    let lr = if bel.key == "DSP1" { 'R' } else { 'L' };
    for i in 0..10 {
        inps.push((
            format!("AD_CPLX_{i}_"),
            &obel_cplx,
            format!("AD_CPLX_DSP{lr}_{i}_"),
        ));
        outps.push(format!("AD_DATA_CPLX_{i}_"));
    }
    for i in 0..18 {
        outps.push(format!("A_CPLX_{i}_"));
        outps.push(format!("B2B1_CPLX_{i}_"));
        outps.push(format!("A_TO_D_CPLX_{i}_"));
        inps.push((
            format!("D_FROM_A_CPLX_{i}_"),
            &obel_odsp,
            format!("A_TO_D_CPLX_{i}_"),
        ));
    }
    for i in 0..37 {
        inps.push((format!("U_CPLX_{i}_"), &obel_cplx, format!("U_CPLX_{i}_")));
        inps.push((format!("V_CPLX_{i}_"), &obel_cplx, format!("V_CPLX_{i}_")));
    }
    outps.push("CONJ_CPLX_OUT".to_string());
    inps.push((
        "CONJ_CPLX_MULT_IN".to_string(),
        &obel_cplx,
        format!("CONJ_DSP_{lr}_MULT_OUT"),
    ));
    inps.push((
        "CONJ_CPLX_PREADD_IN".to_string(),
        &obel_cplx,
        format!("CONJ_DSP_{lr}_PREADD_OUT"),
    ));
    for i in 0..34 {
        cascs.push((format!("ACIN_{i}_"), format!("ACOUT_{i}_")));
    }
    for i in 0..32 {
        cascs.push((format!("BCIN_{i}_"), format!("BCOUT_{i}_")));
    }
    for i in 0..58 {
        cascs.push((format!("PCIN_{i}_"), format!("PCOUT_{i}_")));
    }
    cascs.push(("MULTSIGNIN".to_string(), "MULTSIGNOUT".to_string()));
    cascs.push(("CARRYCASCIN".to_string(), "CARRYCASCOUT".to_string()));
    let mut pins = vec![];
    for out in &outps {
        pins.push((&**out, SitePinDir::Out));
        vrf.claim_node(&[bel.fwire(out)]);
        vrf.claim_node(&[bel.fwire_far(out)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(out), bel.wire(out));
    }
    for (inp, obel, opin) in &inps {
        pins.push((&**inp, SitePinDir::In));
        vrf.claim_node(&[bel.fwire(inp)]);
        vrf.claim_pip(bel.crd(), bel.wire(inp), obel.wire_far(opin));
    }
    let obel_s = vrf.find_bel_delta(bel, 0, -2, bel.key);
    for (ipin, opin) in &cascs {
        pins.push((&**ipin, SitePinDir::In));
        pins.push((&**opin, SitePinDir::Out));
        vrf.claim_node(&[bel.fwire(ipin)]);
        vrf.claim_node(&[bel.fwire(opin)]);
        vrf.claim_node(&[bel.fwire_far(opin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(opin), bel.wire(opin));
        vrf.claim_pip(bel.crd(), bel.wire(ipin), bel.wire_far(ipin));
        if let Some(ref obel) = obel_s {
            vrf.verify_node(&[bel.fwire_far(ipin), obel.fwire_far(opin)]);
        } else {
            vrf.claim_node(&[bel.fwire_far(ipin)]);
        }
    }
    vrf.verify_bel(bel, "DSP58_PRIMARY", &pins, &[]);
}

fn verify_dsp_cplx(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let mut inps = vec![];
    let mut outps = vec![];
    let obel_dsp0 = vrf.find_bel_sibling(bel, "DSP0");
    let obel_dsp1 = vrf.find_bel_sibling(bel, "DSP1");
    for i in 0..10 {
        outps.push(format!("AD_CPLX_DSPL_{i}_"));
        outps.push(format!("AD_CPLX_DSPR_{i}_"));
        inps.push((
            format!("AD_DATA_CPLX_DSPL_{i}_"),
            &obel_dsp0,
            format!("AD_DATA_CPLX_{i}_"),
        ));
        inps.push((
            format!("AD_DATA_CPLX_DSPR_{i}_"),
            &obel_dsp1,
            format!("AD_DATA_CPLX_{i}_"),
        ));
    }
    for i in 0..18 {
        inps.push((format!("A_CPLX_L_{i}_"), &obel_dsp0, format!("A_CPLX_{i}_")));
        inps.push((
            format!("B2B1_CPLX_L_{i}_"),
            &obel_dsp0,
            format!("B2B1_CPLX_{i}_"),
        ));
        inps.push((
            format!("B2B1_CPLX_R_{i}_"),
            &obel_dsp1,
            format!("B2B1_CPLX_{i}_"),
        ));
    }
    for i in 0..37 {
        outps.push(format!("U_CPLX_{i}_"));
        outps.push(format!("V_CPLX_{i}_"));
    }
    inps.push((
        "CONJ_DSP_L_IN".to_string(),
        &obel_dsp0,
        "CONJ_CPLX_OUT".to_string(),
    ));
    inps.push((
        "CONJ_DSP_R_IN".to_string(),
        &obel_dsp1,
        "CONJ_CPLX_OUT".to_string(),
    ));
    outps.push("CONJ_DSP_L_MULT_OUT".to_string());
    outps.push("CONJ_DSP_R_MULT_OUT".to_string());
    outps.push("CONJ_DSP_L_PREADD_OUT".to_string());
    outps.push("CONJ_DSP_R_PREADD_OUT".to_string());
    let mut pins = vec![];
    for out in &outps {
        pins.push((&**out, SitePinDir::Out));
        vrf.claim_node(&[bel.fwire(out)]);
        vrf.claim_node(&[bel.fwire_far(out)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(out), bel.wire(out));
    }
    for (inp, obel, opin) in &inps {
        pins.push((&**inp, SitePinDir::In));
        vrf.claim_node(&[bel.fwire(inp)]);
        vrf.claim_pip(bel.crd(), bel.wire(inp), obel.wire_far(opin));
    }
    vrf.verify_bel(bel, "DSP58_CPLX", &pins, &[]);
}

fn verify_bram_f(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let mut cascs = vec![];
    for ab in ['A', 'B'] {
        for i in 0..32 {
            cascs.push((format!("CASDIN{ab}_{i}_"), format!("CASDOUT{ab}_{i}_")));
        }
        for i in 0..4 {
            cascs.push((format!("CASDINP{ab}_{i}_"), format!("CASDOUTP{ab}_{i}_")));
        }
    }
    cascs.push(("CASINSBITERR".to_string(), "CASOUTSBITERR".to_string()));
    cascs.push(("CASINDBITERR".to_string(), "CASOUTDBITERR".to_string()));
    let mut pins = vec![];
    let obel_s = vrf.find_bel_delta(bel, 0, -4, bel.key);
    for (ipin, opin) in &cascs {
        pins.push((&**ipin, SitePinDir::In));
        pins.push((&**opin, SitePinDir::Out));
        vrf.claim_node(&[bel.fwire(ipin)]);
        vrf.claim_node(&[bel.fwire(opin)]);
        vrf.claim_node(&[bel.fwire_far(opin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(opin), bel.wire(opin));
        vrf.claim_pip(bel.crd(), bel.wire(ipin), bel.wire_far(ipin));
        if let Some(ref obel) = obel_s {
            vrf.verify_node(&[bel.fwire_far(ipin), obel.fwire_far(opin)]);
        } else {
            vrf.claim_node(&[bel.fwire_far(ipin)]);
        }
    }
    vrf.verify_bel(bel, "RAMB36", &pins, &[]);
}

fn verify_bram_h(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let mut inps = vec![];
    let mut outps = vec![];
    let obel_f = vrf.find_bel_sibling(
        bel,
        if bel.key.starts_with("BRAM_L") {
            "BRAM_L_F"
        } else {
            "BRAM_R_F"
        },
    );
    let idx = if bel.key.ends_with('1') { 1 } else { 0 };
    for ab in ['A', 'B'] {
        for i in 0..16 {
            let ii = i * 2 + idx;
            outps.push((format!("CASDOUT{ab}_{i}_"), format!("CASDOUT{ab}_{ii}_")));
            inps.push((format!("CASDIN{ab}_{i}_"), format!("CASDIN{ab}_{ii}_")));
        }
        for i in 0..2 {
            let ii = i * 2 + idx;
            outps.push((format!("CASDOUTP{ab}_{i}_"), format!("CASDOUTP{ab}_{ii}_")));
            inps.push((format!("CASDINP{ab}_{i}_"), format!("CASDINP{ab}_{ii}_")));
        }
    }
    let mut pins = vec![];
    for (ipin, ipin_f) in &inps {
        pins.push((&**ipin, SitePinDir::In));
        vrf.claim_node(&[bel.fwire(ipin)]);
        vrf.claim_pip(bel.crd(), bel.wire(ipin), obel_f.wire_far(ipin_f));
    }
    for (opin, opin_f) in &outps {
        pins.push((&**opin, SitePinDir::Out));
        vrf.claim_node(&[bel.fwire(opin)]);
        vrf.claim_pip(bel.crd(), obel_f.wire_far(opin_f), bel.wire(opin));
    }
    let kind = match bel.key {
        "BRAM_L_H0" | "BRAM_R_H0" => "RAMB18_L",
        "BRAM_L_H1" | "BRAM_R_H1" => "RAMB18_U",
        _ => unreachable!(),
    };
    vrf.verify_bel(bel, kind, &pins, &[]);
}

fn verify_uram(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let mut cascs = vec![];
    for ab in ['A', 'B'] {
        for i in 0..72 {
            cascs.push((
                format!("CAS_IN_DIN_{ab}_{i}_"),
                format!("CAS_OUT_DIN_{ab}_{i}_"),
            ));
            cascs.push((
                format!("CAS_IN_DOUT_{ab}_{i}_"),
                format!("CAS_OUT_DOUT_{ab}_{i}_"),
            ));
        }
        for i in 0..26 {
            cascs.push((
                format!("CAS_IN_ADDR_{ab}_{i}_"),
                format!("CAS_OUT_ADDR_{ab}_{i}_"),
            ));
        }
        for i in 0..9 {
            cascs.push((
                format!("CAS_IN_BWE_{ab}_{i}_"),
                format!("CAS_OUT_BWE_{ab}_{i}_"),
            ));
        }
        for n in ["EN", "SBITERR", "DBITERR", "RDACCESS", "RDB_WR"] {
            cascs.push((format!("CAS_IN_{n}_{ab}"), format!("CAS_OUT_{n}_{ab}")));
        }
    }
    let mut pins = vec![];
    let obel_s = if bel.key == "URAM" {
        vrf.find_bel_delta(bel, 0, -4, "URAM_CAS_DLY")
            .or_else(|| vrf.find_bel_delta(bel, 0, -4, bel.key))
    } else {
        Some(vrf.find_bel_sibling(bel, "URAM"))
    };
    for (ipin, opin) in &cascs {
        pins.push((&**ipin, SitePinDir::In));
        pins.push((&**opin, SitePinDir::Out));
        vrf.claim_node(&[bel.fwire(ipin)]);
        vrf.claim_node(&[bel.fwire(opin)]);
        vrf.claim_node(&[bel.fwire_far(opin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(opin), bel.wire(opin));
        vrf.claim_pip(bel.crd(), bel.wire(ipin), bel.wire_far(ipin));
        if let Some(ref obel) = obel_s {
            vrf.verify_node(&[bel.fwire_far(ipin), obel.fwire_far(opin)]);
        } else {
            vrf.claim_node(&[bel.fwire_far(ipin)]);
        }
    }
    vrf.verify_bel(
        bel,
        if bel.key == "URAM" {
            "URAM288"
        } else {
            bel.key
        },
        &pins,
        &[],
    );
}

fn verify_hardip(
    endev: &ExpandedNamedDevice,
    vrf: &mut Verifier,
    bel: &BelContext<'_>,
    kind: &'static str,
) {
    if endev.edev.disabled.contains(&DisabledPart::HardIpSite(
        bel.die,
        bel.col,
        endev.edev.grids[bel.die].row_to_reg(bel.row),
    )) {
        return;
    }
    vrf.verify_bel(bel, kind, &[], &[]);
}

fn verify_bufdiv_leaf(endev: &ExpandedNamedDevice, vrf: &mut Verifier, bel: &BelContext<'_>) {
    let grid = endev.edev.grids[bel.die];
    let mut pins = vec![("I", SitePinDir::In), ("O_CASC", SitePinDir::Out)];
    if !bel.bel.pins.contains_key("O") {
        pins.push(("O", SitePinDir::Out));
        vrf.claim_node(&[bel.fwire("O")]);
        vrf.claim_node(&[bel.fwire_far("O")]);
        vrf.claim_pip(bel.crd(), bel.wire_far("O"), bel.wire("O"));
    }
    if !bel.bel.pins.contains_key("I_CASC") {
        pins.push(("I_CASC", SitePinDir::In));
        let (key_base, idx) = bel.key.rsplit_once('.').unwrap();
        let idx: u32 = idx.parse().unwrap();
        let obel = vrf.find_bel_sibling(bel, &format!("{key_base}.{oidx}", oidx = idx - 1));
        vrf.claim_node(&[bel.fwire("I_CASC")]);
        vrf.claim_pip(bel.crd(), bel.wire("I_CASC"), obel.wire_far("O_CASC"));
    }
    vrf.verify_bel(
        bel,
        if grid.is_vr {
            "BUFDIV_LEAF_ULVT"
        } else {
            "BUFDIV_LEAF"
        },
        &pins,
        &[],
    );

    vrf.claim_node(&[bel.fwire("O_CASC")]);
    vrf.claim_node(&[bel.fwire_far("O_CASC")]);
    vrf.claim_pip(bel.crd(), bel.wire_far("O_CASC"), bel.wire("O_CASC"));

    vrf.claim_node(&[bel.fwire("I")]);
    vrf.claim_node(&[bel.fwire_far("I")]);
    vrf.claim_pip(bel.crd(), bel.wire("I"), bel.wire_far("I"));
    let (key_hdistr_loc, key_vcc) = if bel.key.starts_with("BUFDIV_LEAF.CLE") {
        ("RCLK_HDISTR_LOC.CLE", "VCC.RCLK_CLE")
    } else if bel.key.starts_with("BUFDIV_LEAF.W") {
        ("RCLK_HDISTR_LOC.W", "VCC.RCLK_INTF.W")
    } else {
        ("RCLK_HDISTR_LOC.E", "VCC.RCLK_INTF.E")
    };
    let obel_hdistr_loc = vrf.find_bel_sibling(bel, key_hdistr_loc);
    let obel_vcc = vrf.find_bel_sibling(bel, key_vcc);
    vrf.claim_pip(bel.crd(), bel.wire_far("I"), obel_vcc.wire("VCC"));
    for i in 0..24 {
        vrf.claim_pip(
            bel.crd(),
            bel.wire_far("I"),
            obel_hdistr_loc.wire(&format!("HDISTR_LOC{i}")),
        );
    }
}

fn verify_rclk_hdistr_loc(
    _endev: &ExpandedNamedDevice,
    _vrf: &mut Verifier,
    _bel: &BelContext<'_>,
) {
    // XXX verify HDISTR_LOC
}

fn verify_hdiob(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let idx: u32 = bel.key[5..].parse().unwrap();
    let obel = vrf.find_bel_sibling(bel, &format!("HDIOLOGIC{idx}"));
    vrf.verify_bel(
        bel,
        "HDIOB",
        &[
            ("RXOUT_M", SitePinDir::Out),
            ("RXOUT_S", SitePinDir::Out),
            ("OP_M", SitePinDir::In),
            ("OP_S", SitePinDir::In),
            ("TRISTATE_M", SitePinDir::In),
            ("TRISTATE_S", SitePinDir::In),
        ],
        &[],
    );
    for opin in ["RXOUT_M", "RXOUT_S"] {
        vrf.claim_node(&[bel.fwire(opin)]);
        vrf.claim_node(&[bel.fwire_far(opin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(opin), bel.wire(opin));
    }
    for (ipin, opin) in [
        ("OP_M", "OPFFM_Q"),
        ("OP_S", "OPFFS_Q"),
        ("TRISTATE_M", "TFFM_Q"),
        ("TRISTATE_S", "TFFS_Q"),
    ] {
        vrf.claim_node(&[bel.fwire(ipin)]);
        vrf.claim_pip(bel.crd(), bel.wire(ipin), obel.wire_far(opin));
    }
}

fn verify_hdiologic(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let idx: u32 = bel.key[9..].parse().unwrap();
    let obel = vrf.find_bel_sibling(bel, &format!("HDIOB{idx}"));
    vrf.verify_bel(
        bel,
        "HDIOLOGIC",
        &[
            ("OPFFM_Q", SitePinDir::Out),
            ("OPFFS_Q", SitePinDir::Out),
            ("TFFM_Q", SitePinDir::Out),
            ("TFFS_Q", SitePinDir::Out),
            ("IPFFM_D", SitePinDir::In),
            ("IPFFS_D", SitePinDir::In),
        ],
        &[],
    );
    for opin in ["OPFFM_Q", "OPFFS_Q", "TFFM_Q", "TFFS_Q"] {
        vrf.claim_node(&[bel.fwire(opin)]);
        vrf.claim_node(&[bel.fwire_far(opin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(opin), bel.wire(opin));
    }
    for (ipin, opin) in [("IPFFM_D", "RXOUT_M"), ("IPFFS_D", "RXOUT_S")] {
        vrf.claim_node(&[bel.fwire(ipin)]);
        vrf.claim_pip(bel.crd(), bel.wire(ipin), obel.wire_far(opin));
    }
}

fn verify_bufgce_hdio(vrf: &mut Verifier, bel: &BelContext<'_>) {
    vrf.verify_bel(
        bel,
        "BUFGCE_HDIO",
        &[("I", SitePinDir::In), ("O", SitePinDir::Out)],
        &[],
    );

    vrf.claim_node(&[bel.fwire("I")]);
    vrf.claim_node(&[bel.fwire_far("I")]);
    vrf.claim_pip(bel.crd(), bel.wire("I"), bel.wire_far("I"));
    let obel_vcc = vrf.find_bel_sibling(bel, "VCC.HDIO");
    vrf.claim_pip(bel.crd(), bel.wire_far("I"), obel_vcc.wire("VCC"));
    let obel_dpll = vrf.find_bel_sibling(bel, "DPLL.HDIO");
    for opin in ["CLKOUT0", "CLKOUT1", "CLKOUT2", "CLKOUT3", "TMUXOUT"] {
        vrf.claim_pip(bel.crd(), bel.wire_far("I"), obel_dpll.wire_far(opin));
    }
    vrf.claim_pip(bel.crd(), bel.wire_far("I"), obel_dpll.wire("CLKIN_INT"));
    let obel_iob_a = vrf.find_bel_sibling(bel, "HDIOB5");
    vrf.claim_pip(bel.crd(), bel.wire_far("I"), obel_iob_a.wire_far("RXOUT_M"));
    let obel_iob_b = vrf.find_bel_sibling(bel, "HDIOB6");
    vrf.claim_pip(bel.crd(), bel.wire_far("I"), obel_iob_b.wire_far("RXOUT_M"));
    for i in 0..8 {
        let pin = format!("I_DUMMY{i}");
        vrf.claim_node(&[bel.fwire(&pin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far("I"), bel.wire(&pin));
    }

    vrf.claim_node(&[bel.fwire("O")]);
    vrf.claim_node(&[bel.fwire_far("O")]);
    vrf.claim_pip(bel.crd(), bel.wire_far("O"), bel.wire("O"));
}

fn verify_dpll_hdio(endev: &ExpandedNamedDevice, vrf: &mut Verifier, bel: &BelContext<'_>) {
    let grid = endev.edev.grids[bel.die];
    let reg = grid.row_to_reg(bel.row);
    if !endev
        .edev
        .disabled
        .contains(&DisabledPart::HdioDpll(bel.die, bel.col, reg))
    {
        vrf.verify_bel(
            bel,
            "DPLL",
            &[
                ("CLKIN", SitePinDir::In),
                ("CLKIN_DESKEW", SitePinDir::In),
                ("CLKOUT0", SitePinDir::Out),
                ("CLKOUT1", SitePinDir::Out),
                ("CLKOUT2", SitePinDir::Out),
                ("CLKOUT3", SitePinDir::Out),
                ("TMUXOUT", SitePinDir::Out),
            ],
            &["CLKIN_INT"],
        );
    }

    for pin in ["CLKIN", "CLKIN_DESKEW"] {
        vrf.claim_node(&[bel.fwire(pin)]);
        vrf.claim_node(&[bel.fwire_far(pin)]);
        vrf.claim_pip(bel.crd(), bel.wire(pin), bel.wire_far(pin));
    }
    vrf.claim_pip(bel.crd(), bel.wire_far("CLKIN"), bel.wire("CLKIN_INT"));
    vrf.claim_pip(bel.crd(), bel.wire_far("CLKIN"), bel.wire("CLKIN_RCLK"));
    let obel_iob_a = vrf.find_bel_sibling(bel, "HDIOB5");
    vrf.claim_pip(
        bel.crd(),
        bel.wire_far("CLKIN"),
        obel_iob_a.wire_far("RXOUT_M"),
    );
    let obel_iob_b = vrf.find_bel_sibling(bel, "HDIOB6");
    vrf.claim_pip(
        bel.crd(),
        bel.wire_far("CLKIN"),
        obel_iob_b.wire_far("RXOUT_M"),
    );
    vrf.claim_pip(
        bel.crd(),
        bel.wire_far("CLKIN_DESKEW"),
        bel.wire_far("CLKIN"),
    );
    vrf.claim_pip(
        bel.crd(),
        bel.wire_far("CLKIN_DESKEW"),
        bel.wire("CLKIN_DESKEW_DUMMY0"),
    );
    vrf.claim_pip(
        bel.crd(),
        bel.wire_far("CLKIN_DESKEW"),
        bel.wire("CLKIN_DESKEW_DUMMY1"),
    );
    vrf.claim_node(&[bel.fwire("CLKIN_DESKEW_DUMMY0")]);
    vrf.claim_node(&[bel.fwire("CLKIN_DESKEW_DUMMY1")]);

    if grid.is_reg_top(reg) {
        let obel = vrf.find_bel_delta(bel, 0, 0, "RCLK_HDIO_DPLL").unwrap();
        vrf.verify_node(&[bel.fwire("CLKIN_RCLK"), obel.fwire("OUT_N")]);
    } else {
        let obel = vrf.find_bel_delta(bel, 0, 48, "RCLK_HDIO_DPLL").unwrap();
        vrf.verify_node(&[bel.fwire("CLKIN_RCLK"), obel.fwire("OUT_S")]);
    }

    for pin in ["CLKOUT0", "CLKOUT1", "CLKOUT2", "CLKOUT3", "TMUXOUT"] {
        vrf.claim_node(&[bel.fwire(pin)]);
        vrf.claim_node(&[bel.fwire_far(pin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(pin), bel.wire(pin));
        vrf.claim_pip(bel.crd(), bel.wire_far("CLKIN_DESKEW"), bel.wire_far(pin));
    }
}

fn verify_dpll_gt(_endev: &ExpandedNamedDevice, vrf: &mut Verifier, bel: &BelContext<'_>) {
    vrf.verify_bel(
        bel,
        "DPLL",
        &[
            ("CLKIN", SitePinDir::In),
            ("CLKIN_DESKEW", SitePinDir::In),
            ("CLKOUT0", SitePinDir::Out),
            ("CLKOUT1", SitePinDir::Out),
            ("CLKOUT2", SitePinDir::Out),
            ("CLKOUT3", SitePinDir::Out),
            ("TMUXOUT", SitePinDir::Out),
        ],
        &[],
    );

    for pin in ["CLKIN", "CLKIN_DESKEW"] {
        vrf.claim_node(&[bel.fwire(pin)]);
        // TODO: source instead
        vrf.claim_node(&[bel.fwire_far(pin)]);
        vrf.claim_pip(bel.crd(), bel.wire(pin), bel.wire_far(pin));
    }

    for pin in ["CLKOUT0", "CLKOUT1", "CLKOUT2", "CLKOUT3", "TMUXOUT"] {
        vrf.claim_node(&[bel.fwire(pin)]);
        vrf.claim_node(&[bel.fwire_far(pin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(pin), bel.wire(pin));
    }
}

fn verify_rclk_hdio_dpll(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let obel_vcc = vrf.find_bel_sibling(bel, "VCC.RCLK_INTF.W");
    let obel_hdistr_loc = vrf.find_bel_sibling(bel, "RCLK_HDISTR_LOC.W");
    for opin in ["OUT_S", "OUT_N"] {
        vrf.claim_node(&[bel.fwire(opin)]);
        vrf.claim_pip(bel.crd(), bel.wire(opin), obel_vcc.wire("VCC"));
        for i in 0..24 {
            vrf.claim_pip(
                bel.crd(),
                bel.wire(opin),
                obel_hdistr_loc.wire(&format!("HDISTR_LOC{i}")),
            );
        }
    }
}

fn verify_rclk_hdio(endev: &ExpandedNamedDevice, vrf: &mut Verifier, bel: &BelContext<'_>) {
    let obel_vcc = vrf.find_bel_sibling(bel, "VCC.RCLK_INTF.W");
    for i in 0..24 {
        let opin = format!("HDISTR{i}");
        let mpin = format!("HDISTR{i}_MUX");
        vrf.claim_pip(bel.crd(), bel.wire(&opin), bel.wire(&mpin));
        vrf.claim_pip(bel.crd(), bel.wire(&opin), obel_vcc.wire("VCC"));
        for j in 0..4 {
            vrf.claim_pip(
                bel.crd(),
                bel.wire(&mpin),
                bel.wire(&format!("BUFGCE_OUT_S{j}")),
            );
            vrf.claim_pip(
                bel.crd(),
                bel.wire(&mpin),
                bel.wire(&format!("BUFGCE_OUT_N{j}")),
            );
        }
    }
    for i in 0..12 {
        let opin = format!("HROUTE{i}");
        let mpin = format!("HROUTE{i}_MUX");
        vrf.claim_pip(bel.crd(), bel.wire(&opin), bel.wire(&mpin));
        vrf.claim_pip(bel.crd(), bel.wire(&opin), obel_vcc.wire("VCC"));
        for j in 0..4 {
            vrf.claim_pip(
                bel.crd(),
                bel.wire(&mpin),
                bel.wire(&format!("BUFGCE_OUT_S{j}")),
            );
            vrf.claim_pip(
                bel.crd(),
                bel.wire(&mpin),
                bel.wire(&format!("BUFGCE_OUT_N{j}")),
            );
        }
    }
    let grid = endev.edev.grids[bel.die];
    let reg = grid.row_to_reg(bel.row);
    for i in 0..4 {
        if let Some(obel) = vrf.find_bel_delta(bel, 0, 0, &format!("BUFGCE_HDIO{i}")) {
            vrf.verify_node(&[bel.fwire(&format!("BUFGCE_OUT_N{i}")), obel.fwire_far("O")]);
        }
    }
    if reg.to_idx() % 2 == 1 {
        for i in 0..4 {
            if let Some(obel) = vrf.find_bel_delta(bel, 0, -48, &format!("BUFGCE_HDIO{i}")) {
                vrf.verify_node(&[bel.fwire(&format!("BUFGCE_OUT_S{i}")), obel.fwire_far("O")]);
            } else {
                vrf.claim_node(&[bel.fwire(&format!("BUFGCE_OUT_S{i}"))]);
            }
        }
    } else {
        for i in 0..4 {
            vrf.claim_node(&[bel.fwire(&format!("BUFGCE_OUT_S{i}"))]);
        }
    }
    // XXX source HDISTR, HROUTE
}

fn verify_rclk_hb_hdio(_endev: &ExpandedNamedDevice, vrf: &mut Verifier, bel: &BelContext<'_>) {
    let obel_vcc = vrf.find_bel_sibling(bel, "VCC.RCLK_INTF.W");
    for i in 0..24 {
        let opin = format!("HDISTR{i}");
        let mpin = format!("HDISTR{i}_MUX");
        vrf.claim_pip(bel.crd(), bel.wire(&opin), bel.wire(&mpin));
        vrf.claim_pip(bel.crd(), bel.wire(&opin), obel_vcc.wire("VCC"));
        for j in 0..4 {
            vrf.claim_pip(
                bel.crd(),
                bel.wire(&mpin),
                bel.wire(&format!("BUFGCE_OUT_S{j}")),
            );
            vrf.claim_pip(
                bel.crd(),
                bel.wire(&mpin),
                bel.wire(&format!("HDISTR{i}_MUX_DUMMY{j}")),
            );
        }
    }
    for i in 0..12 {
        let opin = format!("HROUTE{i}");
        let mpin = format!("HROUTE{i}_MUX");
        vrf.claim_pip(bel.crd(), bel.wire(&opin), bel.wire(&mpin));
        vrf.claim_pip(bel.crd(), bel.wire(&opin), obel_vcc.wire("VCC"));
        for j in 0..4 {
            vrf.claim_pip(
                bel.crd(),
                bel.wire(&mpin),
                bel.wire(&format!("BUFGCE_OUT_S{j}")),
            );
            vrf.claim_pip(
                bel.crd(),
                bel.wire(&mpin),
                bel.wire(&format!("HROUTE{i}_MUX_DUMMY{j}")),
            );
        }
    }
    for i in 0..4 {
        if let Some(obel) = vrf.find_bel_delta(bel, 0, -48, &format!("BUFGCE_HDIO{i}")) {
            vrf.verify_node(&[bel.fwire(&format!("BUFGCE_OUT_S{i}")), obel.fwire_far("O")]);
        } else {
            vrf.claim_node(&[bel.fwire(&format!("BUFGCE_OUT_S{i}"))]);
        }
    }
    // XXX source HDISTR, HROUTE
}

fn verify_vnoc_nxu512(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let (kind, obel_key, obel_pin) = match bel.key {
        "VNOC_NSU512" => ("NOC_NSU512", "VNOC_NPS_A", "OUT_3"),
        "VNOC_NMU512" => ("NOC_NMU512", "VNOC_NPS_B", "OUT_3"),
        "VNOC2_NSU512" => ("NOC2_NSU512", "VNOC2_NPS_A", "OUT_3"),
        "VNOC2_NMU512" => ("NOC2_NMU512", "VNOC2_NPS_B", "OUT_3"),
        "VNOC4_NSU512" => ("NOC2_NSU512", "VNOC4_NPS_A", "OUT_3"),
        "VNOC4_NMU512" => ("NOC2_NMU512", "VNOC4_NPS_B", "OUT_0"),
        _ => unreachable!(),
    };
    vrf.verify_bel(
        bel,
        kind,
        &[("TO_NOC", SitePinDir::Out), ("FROM_NOC", SitePinDir::In)],
        &[],
    );
    vrf.claim_node(&[bel.fwire("TO_NOC")]);
    vrf.claim_node(&[bel.fwire("FROM_NOC")]);
    vrf.claim_node(&[bel.fwire_far("TO_NOC")]);
    let obel = vrf.find_bel_sibling(bel, obel_key);
    vrf.verify_node(&[bel.fwire_far("FROM_NOC"), obel.fwire_far(obel_pin)]);
    vrf.claim_pip(bel.crd(), bel.wire_far("TO_NOC"), bel.wire("TO_NOC"));
    vrf.claim_pip(bel.crd(), bel.wire("FROM_NOC"), bel.wire_far("FROM_NOC"));
}

fn verify_vnoc_nps(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let (kind, obel_key_nxu, obel_key_nps) = match bel.key {
        "VNOC_NPS_A" => ("NOC_NPS_VNOC", "VNOC_NSU512", "VNOC_NPS_B"),
        "VNOC_NPS_B" => ("NOC_NPS_VNOC", "VNOC_NMU512", "VNOC_NPS_A"),
        "VNOC2_NPS_A" => ("NOC2_NPS5555", "VNOC2_NSU512", "VNOC2_NPS_B"),
        "VNOC2_NPS_B" => ("NOC2_NPS5555", "VNOC2_NMU512", "VNOC2_NPS_A"),
        _ => unreachable!(),
    };
    vrf.verify_bel(
        bel,
        kind,
        &[
            ("OUT_0", SitePinDir::Out),
            ("OUT_1", SitePinDir::Out),
            ("OUT_2", SitePinDir::Out),
            ("OUT_3", SitePinDir::Out),
            ("IN_0", SitePinDir::In),
            ("IN_1", SitePinDir::In),
            ("IN_2", SitePinDir::In),
            ("IN_3", SitePinDir::In),
        ],
        &[],
    );
    for pin in ["OUT_0", "OUT_1", "OUT_2", "OUT_3"] {
        vrf.claim_node(&[bel.fwire(pin)]);
        vrf.claim_node(&[bel.fwire_far(pin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(pin), bel.wire(pin));
    }
    for pin in ["IN_0", "IN_1", "IN_2", "IN_3"] {
        vrf.claim_node(&[bel.fwire(pin)]);
        vrf.claim_pip(bel.crd(), bel.wire(pin), bel.wire_far(pin));
    }
    let obel_nxu = vrf.find_bel_sibling(bel, obel_key_nxu);
    let obel_nps = vrf.find_bel_sibling(bel, obel_key_nps);
    vrf.verify_node(&[bel.fwire_far("IN_3"), obel_nxu.fwire_far("TO_NOC")]);
    vrf.verify_node(&[bel.fwire_far("IN_1"), obel_nps.fwire_far("OUT_1")]);
    if bel.key.ends_with('A') {
        if let Some(obel_s) = vrf.find_bel_delta(bel, 0, -48, bel.key) {
            vrf.verify_node(&[bel.fwire_far("IN_0"), obel_s.fwire_far("OUT_2")]);
        } else {
            vrf.claim_node(&[bel.fwire_far("IN_0")]);
        }
        if let Some(obel_n) = vrf.find_bel_delta(bel, 0, 48, bel.key) {
            vrf.verify_node(&[bel.fwire_far("IN_2"), obel_n.fwire_far("OUT_0")]);
        } else {
            vrf.claim_node(&[bel.fwire_far("IN_2")]);
        }
    } else {
        if let Some(obel_s) = vrf.find_bel_delta(bel, 0, -48, bel.key) {
            vrf.verify_node(&[bel.fwire_far("IN_2"), obel_s.fwire_far("OUT_0")]);
        } else {
            vrf.claim_node(&[bel.fwire_far("IN_2")]);
        }
        if let Some(obel_n) = vrf.find_bel_delta(bel, 0, 48, bel.key) {
            vrf.verify_node(&[bel.fwire_far("IN_0"), obel_n.fwire_far("OUT_2")]);
        } else {
            vrf.claim_node(&[bel.fwire_far("IN_0")]);
        }
    }
}

fn verify_vnoc_nps6x(vrf: &mut Verifier, bel: &BelContext<'_>) {
    vrf.verify_bel(
        bel,
        "NOC2_NPS6X",
        &[
            ("OUT_0", SitePinDir::Out),
            ("OUT_1", SitePinDir::Out),
            ("OUT_2", SitePinDir::Out),
            ("OUT_3", SitePinDir::Out),
            ("OUT_4", SitePinDir::Out),
            ("OUT_5", SitePinDir::Out),
            ("IN_0", SitePinDir::In),
            ("IN_1", SitePinDir::In),
            ("IN_2", SitePinDir::In),
            ("IN_3", SitePinDir::In),
            ("IN_4", SitePinDir::In),
            ("IN_5", SitePinDir::In),
        ],
        &[],
    );
    for pin in ["OUT_0", "OUT_1", "OUT_2", "OUT_3", "OUT_4", "OUT_5"] {
        vrf.claim_node(&[bel.fwire(pin)]);
        vrf.claim_node(&[bel.fwire_far(pin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(pin), bel.wire(pin));
    }
    for pin in ["IN_0", "IN_1", "IN_2", "IN_3", "IN_4", "IN_5"] {
        vrf.claim_node(&[bel.fwire(pin)]);
        vrf.claim_pip(bel.crd(), bel.wire(pin), bel.wire_far(pin));
    }
    if bel.key == "VNOC4_NPS_A" {
        let obel_nxu = vrf.find_bel_sibling(bel, "VNOC4_NSU512");
        let obel_nps = vrf.find_bel_sibling(bel, "VNOC4_NPS_B");
        vrf.verify_node(&[bel.fwire_far("IN_3"), obel_nxu.fwire_far("TO_NOC")]);
        vrf.verify_node(&[bel.fwire_far("IN_0"), obel_nps.fwire_far("OUT_3")]);
    } else {
        let obel_nxu = vrf.find_bel_sibling(bel, "VNOC4_NMU512");
        let obel_nps = vrf.find_bel_sibling(bel, "VNOC4_NPS_A");
        vrf.verify_node(&[bel.fwire_far("IN_0"), obel_nxu.fwire_far("TO_NOC")]);
        vrf.verify_node(&[bel.fwire_far("IN_3"), obel_nps.fwire_far("OUT_0")]);
    }
    if let Some(obel_s) = vrf.find_bel_delta(bel, 0, -48, bel.key) {
        vrf.verify_node(&[bel.fwire_far("IN_4"), obel_s.fwire_far("OUT_2")]);
        vrf.verify_node(&[bel.fwire_far("IN_5"), obel_s.fwire_far("OUT_1")]);
    } else {
        vrf.claim_node(&[bel.fwire_far("IN_4")]);
        vrf.claim_node(&[bel.fwire_far("IN_5")]);
    }
    if let Some(obel_n) = vrf.find_bel_delta(bel, 0, 48, bel.key) {
        vrf.verify_node(&[bel.fwire_far("IN_2"), obel_n.fwire_far("OUT_4")]);
        vrf.verify_node(&[bel.fwire_far("IN_1"), obel_n.fwire_far("OUT_5")]);
    } else {
        vrf.claim_node(&[bel.fwire_far("IN_2")]);
        vrf.claim_node(&[bel.fwire_far("IN_1")]);
    }
}

fn verify_vnoc_scan(vrf: &mut Verifier, bel: &BelContext<'_>) {
    let mut outps = vec![];
    let mut inps = vec![];
    if bel.key == "VNOC2_SCAN" {
        for i in 6..15 {
            outps.push(format!("NOC2_SCAN_CHNL_TO_PL_{i}_"));
            inps.push(format!("NOC2_SCAN_CHNL_FROM_PL_{i}_"))
        }
        for i in 5..14 {
            inps.push(format!("NOC2_SCAN_CHNL_MASK_FROM_PL_{i}_"));
        }
    } else {
        for i in 7..15 {
            outps.push(format!("NOC2_SCAN_CHNL_TO_PL_{i}_"));
            inps.push(format!("NOC2_SCAN_CHNL_FROM_PL_{i}_"))
        }
        for i in 7..14 {
            inps.push(format!("NOC2_SCAN_CHNL_MASK_FROM_PL_{i}_"));
        }
    }
    let mut pins = vec![];
    for ipin in &inps {
        vrf.claim_node(&[bel.fwire(ipin)]);
        vrf.claim_node(&[bel.fwire_far(ipin)]);
        vrf.claim_pip(bel.crd(), bel.wire(ipin), bel.wire_far(ipin));
        pins.push((&**ipin, SitePinDir::In));
    }
    for opin in &outps {
        vrf.claim_node(&[bel.fwire(opin)]);
        vrf.claim_node(&[bel.fwire_far(opin)]);
        vrf.claim_pip(bel.crd(), bel.wire_far(opin), bel.wire(opin));
        pins.push((&**opin, SitePinDir::Out));
    }
    vrf.verify_bel(bel, "NOC2_SCAN", &pins, &[]);
}

fn verify_vdu(vrf: &mut Verifier, bel: &BelContext<'_>) {
    vrf.verify_bel(
        bel,
        "VDU",
        &[
            ("VDUCORECLK", SitePinDir::In),
            ("VDUMCUCLK", SitePinDir::In),
        ],
        &[],
    );
    let obel = vrf.find_bel_sibling(bel, "DPLL.RGT");
    for (pin, pllpin) in [("VDUCORECLK", "CLKOUT2"), ("VDUMCUCLK", "CLKOUT3")] {
        vrf.claim_node(&[bel.fwire(pin)]);
        vrf.claim_pip(bel.crd(), bel.wire(pin), bel.wire_far(pin));
        vrf.verify_node(&[bel.fwire_far(pin), obel.fwire_far(pllpin)]);
    }
}

fn verify_vcc(vrf: &mut Verifier, bel: &BelContext<'_>) {
    vrf.claim_vcc_node(bel.fwire("VCC"));
}

fn verify_bel(endev: &ExpandedNamedDevice, vrf: &mut Verifier, bel: &BelContext<'_>) {
    match bel.key {
        "LAGUNA" => verify_laguna(endev, vrf, bel),
        "DSP0" | "DSP1" => verify_dsp(vrf, bel),
        "DSP_CPLX" => verify_dsp_cplx(vrf, bel),
        "BRAM_L_F" | "BRAM_R_F" => verify_bram_f(vrf, bel),
        "BRAM_L_H0" | "BRAM_L_H1" | "BRAM_R_H0" | "BRAM_R_H1" => verify_bram_h(vrf, bel),
        "URAM" | "URAM_CAS_DLY" => verify_uram(vrf, bel),
        "PCIE4" => verify_hardip(endev, vrf, bel, "PCIE40"),
        "PCIE5" => verify_hardip(endev, vrf, bel, "PCIE50"),
        "MRMAC" => verify_hardip(endev, vrf, bel, "MRMAC"),
        "DCMAC" => verify_hardip(endev, vrf, bel, "DCMAC"),
        "ILKN" => verify_hardip(endev, vrf, bel, "ILKNF"),
        "HSC" => verify_hardip(endev, vrf, bel, "HSC"),
        "SDFEC" => verify_hardip(endev, vrf, bel, "SDFEC_A"),
        "DFE_CFC_BOT" => verify_hardip(endev, vrf, bel, "DFE_CFC_BOT"),
        "DFE_CFC_TOP" => verify_hardip(endev, vrf, bel, "DFE_CFC_TOP"),
        "RCLK_DFX_TEST.E" | "RCLK_DFX_TEST.W" => vrf.verify_bel(bel, "RCLK_DFX_TEST", &[], &[]),
        "SYSMON_SAT.VNOC" | "SYSMON_SAT.LGT" | "SYSMON_SAT.RGT" => {
            vrf.verify_bel(bel, "SYSMON_SAT", &[], &[])
        }
        "DPLL.HDIO" => verify_dpll_hdio(endev, vrf, bel),
        "DPLL.LGT" | "DPLL.RGT" => verify_dpll_gt(endev, vrf, bel),
        "RCLK_HDIO_DPLL" => verify_rclk_hdio_dpll(vrf, bel),
        "RCLK_HDIO" => verify_rclk_hdio(endev, vrf, bel),
        "RCLK_HB_HDIO" => verify_rclk_hb_hdio(endev, vrf, bel),
        "VNOC_NSU512" | "VNOC_NMU512" | "VNOC2_NSU512" | "VNOC2_NMU512" | "VNOC4_NSU512"
        | "VNOC4_NMU512" => verify_vnoc_nxu512(vrf, bel),
        "VNOC_NPS_A" | "VNOC_NPS_B" | "VNOC2_NPS_A" | "VNOC2_NPS_B" => verify_vnoc_nps(vrf, bel),
        "VNOC4_NPS_A" | "VNOC4_NPS_B" => verify_vnoc_nps6x(vrf, bel),
        "VNOC2_SCAN" | "VNOC4_SCAN" => verify_vnoc_scan(vrf, bel),
        "HDIO_BIAS" | "RPI_HD_APB" | "HDLOGIC_APB" | "MISR" | "BFR_B" => {
            vrf.verify_bel(bel, bel.key, &[], &[])
        }
        "VDU" => verify_vdu(vrf, bel),

        _ if bel.key.starts_with("SLICE") => verify_slice(vrf, bel),
        _ if bel.key.starts_with("BUFDIV_LEAF") => verify_bufdiv_leaf(endev, vrf, bel),
        _ if bel.key.starts_with("RCLK_HDISTR_LOC") => verify_rclk_hdistr_loc(endev, vrf, bel),
        _ if bel.key.starts_with("HDIOB") => verify_hdiob(vrf, bel),
        _ if bel.key.starts_with("HDIOLOGIC") => verify_hdiologic(vrf, bel),
        _ if bel.key.starts_with("BUFGCE_HDIO") => verify_bufgce_hdio(vrf, bel),
        _ if bel.key.starts_with("VCC") => verify_vcc(vrf, bel),
        _ => println!("MEOW {} {:?}", bel.key, bel.name),
    }
}

fn verify_extra(_endev: &ExpandedNamedDevice, vrf: &mut Verifier) {
    // XXX
    vrf.skip_residual();
}

pub fn verify_device(endev: &ExpandedNamedDevice, rd: &Part) {
    verify(
        rd,
        &endev.ngrid,
        |_| (),
        |vrf, bel| verify_bel(endev, vrf, bel),
        |vrf| verify_extra(endev, vrf),
    );
}
