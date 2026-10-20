use lis_au::*;
use lis_core::Phi;
use proptest::prelude::*;

fn config() -> AuConfig {
    AuConfig {
        framing: FramingConfig {
            start: vec![2],
            end: vec![3],
            bcc: true,
            use_etb: true,
            max_wire_len: 256,
            class: TransmissionClass::B,
        },
        format: FormatConfig {
            unit_no: true,
            rack_digits: 4,
            sample_id_digits: 8,
            sex: false,
            age: false,
            month: false,
            patient_info_digits: [0; 6],
            result_digits: 6,
        },
    }
}
#[cfg(test)]
fn parsed(config: &AuConfig, text: &[u8], end: FrameEnd) -> Message {
    let wire = encode_frame(config, text, end).unwrap();
    let mut decoder = StreamDecoder::new(config.clone()).unwrap();
    let events = decoder.push(&wire);
    let WireEvent::Frame(frame) = events[0].as_ref().unwrap() else {
        panic!("expected frame");
    };
    parse_message(config, frame).unwrap()
}
#[cfg(test)]
fn result(config: &AuConfig, block: u8, end: FrameEnd) -> ResultBlock {
    // Hand-authored synthetic D-space fixture: pp10,14 (not copied from manual).
    let mut text = b"D 01000101U0001SYN00001    E01012.34  ".to_vec();
    text[27] = block;
    let Message::ResultBlock(result) = parsed(config, &text, end) else {
        panic!("result");
    };
    result
}

#[test]
fn hand_calculated_xor_and_chunking() {
    // Manual p5: XOR excludes STX and includes text + ETX.
    let wire = [2, b'D', b'B', b'0', b'1', 3, 4];
    let mut decoder = StreamDecoder::new(config()).unwrap();
    for byte in &wire[..wire.len() - 1] {
        assert!(decoder.push(&[*byte]).is_empty());
    }
    let events = decoder.push(&wire[wire.len() - 1..]);
    let WireEvent::Frame(frame) = events[0].as_ref().unwrap() else {
        panic!("frame");
    };
    assert_eq!(frame.text.expose(), b"DB01");
    assert!(matches!(
        parse_message(&config(), frame).unwrap(),
        Message::Session {
            kind: SessionKind::ResultsStart,
            ..
        }
    ));
    assert!(!decoder.is_pending());
}

#[test]
fn exact_patient_value_flags_and_redaction() {
    let result = result(&config(), b'E', FrameEnd::Text);
    assert_eq!(result.header.sample_id.expose(), b"SYN00001");
    assert_eq!(result.tests[0].value.expose(), b"012.34");
    assert_eq!(result.tests[0].flags.expose(), b"  ");
    assert!(!format!("{result:?}").contains("SYN00001"));
    assert!(!format!("{result:?}").contains("012.34"));
}

#[test]
fn assembly_sequence_retry_conflicts_and_completion() {
    let cfg = config();
    let mut assembler = ResultAssembler::new();
    let zero = result(&cfg, b'0', FrameEnd::Block);
    assert_eq!(
        assembler.push(zero.clone()).unwrap(),
        AssemblyOutcome::Pending
    );
    assert_eq!(
        assembler.push(zero.clone()).unwrap(),
        AssemblyOutcome::Duplicate
    );
    assert_eq!(assembler.finish(), Err(AuError::IncompleteSample));
    let mut conflict = zero;
    conflict.tests[0].value = Phi::new(b"999999".to_vec());
    assert_eq!(assembler.push(conflict), Err(AuError::BlockConflict));
    assert_eq!(
        assembler.push(result(&cfg, b'2', FrameEnd::Block)),
        Err(AuError::BlockSequence)
    );
    let mut changed_header = result(&cfg, b'1', FrameEnd::Block);
    changed_header.header.sample_id = Phi::new(b"SYN00002".to_vec());
    assert_eq!(assembler.push(changed_header), Err(AuError::HeaderConflict));
    assert_eq!(
        assembler.push(result(&cfg, b'1', FrameEnd::Block)).unwrap(),
        AssemblyOutcome::Pending
    );
    let AssemblyOutcome::Complete(sample) =
        assembler.push(result(&cfg, b'E', FrameEnd::Text)).unwrap()
    else {
        panic!("complete");
    };
    assert_eq!(sample.tests.len(), 3);
    assert_eq!(assembler.finish(), Ok(()));
    // No final-frame deduplication: receipt/session context belongs to engine.
    assert!(matches!(
        assembler.push(result(&cfg, b'E', FrameEnd::Text)).unwrap(),
        AssemblyOutcome::Complete(_)
    ));
}

#[test]
fn numeric_and_final_terminators_must_match() {
    let cfg = config();
    let wire = encode_frame(
        &cfg,
        b"D 01000101U0001SYN00001    001012.34  ",
        FrameEnd::Text,
    )
    .unwrap();
    let WireEvent::Frame(frame) = StreamDecoder::new(cfg.clone())
        .unwrap()
        .push(&wire)
        .remove(0)
        .unwrap()
    else {
        panic!("frame");
    };
    assert_eq!(parse_message(&cfg, &frame), Err(AuError::BlockTerminator));
}

#[test]
fn query_normal_and_rerun_session_and_nonpatient_layouts() {
    let cfg = config();
    for code in [b"RB", b"RE", b"DB", b"DE"] {
        let text = [code.as_slice(), b"01"].concat();
        assert!(matches!(
            parsed(&cfg, &text, FrameEnd::Text),
            Message::Session { .. }
        ));
    }
    for code in [b"R ", b"RH"] {
        let text = [code.as_slice(), b"01000101U0001SYN00001"].concat();
        assert!(matches!(
            parsed(&cfg, &text, FrameEnd::Text),
            Message::Query(_)
        ));
    }
    for (text, kind) in [
        (
            b"DR01       R001            E01999999ab".as_slice(),
            ResultKind::ReagentBlank,
        ),
        (
            b"DA01       A001CAL00001  01E01999999ab",
            ResultKind::Calibration,
        ),
        (
            b"DQ01       Q001QC000001  01E01999999ab",
            ResultKind::QualityControl,
        ),
    ] {
        let Message::ResultBlock(block) = parsed(&cfg, text, FrameEnd::Text) else {
            panic!("result");
        };
        assert_eq!(block.kind, kind);
        assert_eq!(block.tests[0].value.expose(), b"999999");
        assert_eq!(block.tests[0].flags.expose(), b"ab");
    }
}

#[test]
fn stat_rerun_and_nine_digit_values_are_opaque() {
    let mut cfg = config();
    cfg.format.result_digits = 9;
    // Manual pp11–12: stat rack is spaces; rerun has original sample number.
    for (text, kind) in [
        (
            b"d 01    22UP001SYN00001    E01000100200  ".as_slice(),
            ResultKind::Stat,
        ),
        (
            b"dH01    22UP002SYN00001P001E01000100200  ",
            ResultKind::StatRerun,
        ),
        (
            b"DH01000101U0002SYN000010001E01000100200  ",
            ResultKind::Rerun,
        ),
    ] {
        let Message::ResultBlock(block) = parsed(&cfg, text, FrameEnd::Text) else {
            panic!("result");
        };
        assert_eq!(block.kind, kind);
        assert_eq!(block.tests[0].value.expose(), b"000100200");
    }
}

#[test]
fn tri_has_no_unit_no_and_splits_whole_items() {
    let cfg = config();
    let block = result(&cfg, b'E', FrameEnd::Text);
    let mut header = block.header;
    header.demographics = Some(Demographics {
        sex: Phi::new(vec![]),
        age: Phi::new(vec![]),
        month: Phi::new(vec![]),
        information: std::array::from_fn(|_| Phi::new(vec![])),
    });
    let tri = Tri {
        rerun: false,
        header,
        tests: (0..250)
            .map(|_| TriTest {
                code: Phi::new(b"01".to_vec()),
                method: None,
            })
            .collect(),
    };
    let wires = encode_tri(&cfg, &tri).unwrap();
    assert!(wires.len() > 1);
    let mut tests = 0;
    for (index, wire) in wires.iter().enumerate() {
        assert!(wire.len() <= 256);
        let WireEvent::Frame(frame) = StreamDecoder::new(cfg.clone())
            .unwrap()
            .push(wire)
            .remove(0)
            .unwrap()
        else {
            panic!("frame");
        };
        assert!(frame.text.expose().starts_with(b"S 000101U0001"));
        let Message::TriBlock(block) = parse_message(&cfg, &frame).unwrap() else {
            panic!("tri");
        };
        assert!(block.header.unit_no.is_none());
        assert_eq!(
            block.block,
            if index + 1 == wires.len() {
                BlockId::Final
            } else {
                BlockId::Intermediate(index as u8)
            }
        );
        tests += block.tests.len();
    }
    assert_eq!(tests, 250);
    let empty = Tri {
        tests: Vec::new(),
        ..tri
    };
    assert_eq!(encode_tri(&cfg, &empty), Err(AuError::Length));
    let stop = encode_stop(&cfg).unwrap();
    assert_eq!(&stop[1..3], b"SE");
    let WireEvent::Frame(frame) = StreamDecoder::new(cfg.clone())
        .unwrap()
        .push(&stop)
        .remove(0)
        .unwrap()
    else {
        panic!("frame");
    };
    assert_eq!(
        parse_message(&cfg, &frame).unwrap(),
        Message::Session {
            kind: SessionKind::TriStop,
            unit_no: None
        }
    );
}

#[test]
fn custom_controls_without_bcc_or_etb() {
    let mut cfg = config();
    cfg.framing.start = vec![1, 2];
    cfg.framing.end = vec![13, 10];
    cfg.framing.bcc = false;
    let wire = encode_frame(&cfg, b"RB01", FrameEnd::Text).unwrap();
    assert_eq!(wire, b"\x01\x02RB01\r\n");
    let mut decoder = StreamDecoder::new(cfg.clone()).unwrap();
    assert!(decoder.push(&wire[..1]).is_empty());
    assert_eq!(decoder.push(&wire[1..]).len(), 1);
    let numeric = result(&cfg, b'0', FrameEnd::Text);
    assert_eq!(numeric.block, BlockId::Intermediate(0));
}

#[test]
fn invalid_profiles_and_unknown_fields_are_rejected() {
    let cfg = config();
    let mut bad = cfg.clone();
    bad.format.sample_id_digits = 0;
    assert_eq!(bad.validate(), Err(AuError::Configuration));
    bad = cfg.clone();
    bad.framing.start = vec![2, 2, 2];
    assert_eq!(bad.validate(), Err(AuError::Configuration));
    bad = cfg.clone();
    bad.framing.max_wire_len = 255;
    assert_eq!(bad.validate(), Err(AuError::Configuration));
    let mut json = serde_json::to_value(&cfg).unwrap();
    json["unexpected"] = serde_json::json!(true);
    assert!(serde_json::from_value::<AuConfig>(json).is_err());
    use ts_rs::TS;
    assert!(AuConfig::decl(&ts_rs::Config::default()).contains("AuConfig"));
}

#[test]
fn invalid_bcc_length_field_and_incomplete_wire_fail() {
    let cfg = config();
    let mut wire = encode_frame(&cfg, b"RB01", FrameEnd::Text).unwrap();
    *wire.last_mut().unwrap() ^= 1;
    assert_eq!(
        StreamDecoder::new(cfg.clone()).unwrap().push(&wire),
        vec![Err(AuError::Checksum)]
    );
    let mut decoder = StreamDecoder::new(cfg).unwrap();
    decoder.push(b"\x02D");
    assert_eq!(decoder.finish(), Err(AuError::Framing));
    assert!(!decoder.is_pending());
}

proptest! {
    #[test]
    fn decoder_and_parser_are_bounded_and_do_not_panic(bytes in prop::collection::vec(any::<u8>(),0..8192), chunk_size in 1usize..128) {
        let cfg=config();let mut decoder=StreamDecoder::new(cfg.clone()).unwrap();
        for chunk in bytes.chunks(chunk_size) {
            for event in decoder.push(chunk) {
                if let Ok(WireEvent::Frame(frame))=event { let _parsed=parse_message(&cfg,&frame); }
            }
            prop_assert!(decoder.buffered_len()<=256);
        }
        let _end=decoder.finish();
    }
    #[test]
    fn fixed_width_parser_does_not_panic(bytes in prop::collection::vec(32u8..=126,0..1200)) {
        let frame=Frame {wire:Phi::new(vec![]),text:Phi::new(bytes),end:FrameEnd::Text};
        let _parsed=parse_message(&config(),&frame);
    }
}

#[test]
fn rack_profiles_and_enabled_demographics_have_exact_widths() {
    let mut cfg = config();
    cfg.format.rack_digits = 0;
    let Message::Query(query) = parsed(&cfg, b"R 01U0001SYN00001", FrameEnd::Text) else {
        panic!("query");
    };
    assert!(query.header.rack.expose().is_empty());
    assert!(query.header.cup.expose().is_empty());
    cfg.format.rack_digits = 5;
    cfg.format.unit_no = false;
    let Message::Query(query) = parsed(&cfg, b"RH0000101U0001SYN00001", FrameEnd::Text) else {
        panic!("query");
    };
    assert_eq!(query.header.rack.expose(), b"00001");
    assert!(query.header.unit_no.is_none());
    cfg = config();
    cfg.format.sex = true;
    cfg.format.age = true;
    cfg.format.month = true;
    cfg.format.patient_info_digits = [2, 0, 0, 0, 0, 0];
    let Message::ResultBlock(block) = parsed(
        &cfg,
        b"D 01000101U0001SYN00001    EM03211AB01012.34  ",
        FrameEnd::Text,
    ) else {
        panic!("result");
    };
    let demographic = block.header.demographics.unwrap();
    assert_eq!(demographic.sex.expose(), b"M");
    assert_eq!(demographic.age.expose(), b"032");
    assert_eq!(demographic.month.expose(), b"11");
    assert_eq!(demographic.information[0].expose(), b"AB");
}

#[test]
fn malformed_fields_partial_items_and_unsupported_nonpatient_profile_fail() {
    let cfg = config();
    for text in [
        b"D 01000101U0001SYN00001    Z01012.34  ".as_slice(),
        b"D 01000101U0001SYN00001    E00012.34  ",
        b"D 01000101U0001SYN00001    E01012.34 ",
        b"D AA000101U0001SYN00001    E01012.34  ",
    ] {
        let frame = Frame {
            wire: Phi::new(vec![]),
            text: Phi::new(text.to_vec()),
            end: FrameEnd::Text,
        };
        assert!(parse_message(&cfg, &frame).is_err());
    }
    let mut unsupported = cfg;
    unsupported.format.rack_digits = 0;
    let frame = Frame {
        wire: Phi::new(vec![]),
        text: Phi::new(b"DQ01 Q001QC000001  01E01999999ab".to_vec()),
        end: FrameEnd::Text,
    };
    assert_eq!(
        parse_message(&unsupported, &frame),
        Err(AuError::Configuration)
    );
}

#[test]
fn rerun_tri_preserves_original_sample_and_explicit_method() {
    let cfg = config();
    let Message::ResultBlock(block) = parsed(
        &cfg,
        b"DH01000101U0002SYN000010001E01012.34  ",
        FrameEnd::Text,
    ) else {
        panic!("result");
    };
    let tri = Tri {
        rerun: true,
        header: block.header,
        tests: vec![TriTest {
            code: Phi::new(b"01".to_vec()),
            method: Some(b'2'),
        }],
    };
    let wires = encode_tri(&cfg, &tri).unwrap();
    let WireEvent::Frame(frame) = StreamDecoder::new(cfg.clone())
        .unwrap()
        .push(&wires[0])
        .remove(0)
        .unwrap()
    else {
        panic!("frame");
    };
    assert_eq!(frame.text.expose(), b"SH000101U0002SYN000010001E012");
    let Message::TriBlock(block) = parse_message(&cfg, &frame).unwrap() else {
        panic!("tri");
    };
    assert_eq!(block.tests[0].method, Some(b'2'));
    assert_eq!(block.header.reserved.expose(), b"0001");
    let mut invalid = tri;
    invalid.tests[0].method = None;
    assert_eq!(encode_tri(&cfg, &invalid), Err(AuError::Field));
}

#[test]
fn decoder_limits_resynchronizes_and_preserves_control_events() {
    let mut cfg = config();
    cfg.framing.start = vec![1, 2];
    let wire = encode_frame(&cfg, b"RB01", FrameEnd::Text).unwrap();
    let mut decoder = StreamDecoder::new(cfg).unwrap();
    assert!(decoder.push(&[1]).is_empty());
    // The second 01 begins a fresh two-byte start after the incomplete prefix.
    let mut overlapping = vec![1];
    overlapping.extend_from_slice(&wire[1..]);
    let events = decoder.push(&overlapping);
    assert!(events[0].is_err());
    assert!(matches!(events[1], Ok(WireEvent::Frame(_))));
    assert_eq!(
        decoder.push(&[6, 21]),
        vec![Ok(WireEvent::Ack), Ok(WireEvent::Nak)]
    );
    let mut decoder = StreamDecoder::new(config()).unwrap();
    let oversized = [vec![2], vec![b'X'; 256]].concat();
    assert!(
        decoder
            .push(&oversized)
            .contains(&Err(AuError::FrameTooLong))
    );
    assert!(!decoder.is_pending());
    for maximum in [256, 512, 1024] {
        let mut cfg = config();
        cfg.framing.max_wire_len = maximum;
        let text = vec![b'X'; usize::from(maximum) - 3];
        assert_eq!(
            encode_frame(&cfg, &text, FrameEnd::Text).unwrap().len(),
            usize::from(maximum)
        );
        let too_long = vec![b'X'; usize::from(maximum) - 2];
        assert_eq!(
            encode_frame(&cfg, &too_long, FrameEnd::Text),
            Err(AuError::FrameTooLong)
        );
    }
}

#[test]
fn matching_control_prefixes_and_etb_start_are_unambiguous_inside_frames() {
    for (start, end) in [
        (vec![2, 1], vec![2, 3]),
        (vec![23], vec![3]),
        (vec![2], vec![2]),
    ] {
        let mut cfg = config();
        cfg.framing.start = start;
        cfg.framing.end = end;
        assert!(matches!(
            parsed(&cfg, b"RB01", FrameEnd::Text),
            Message::Session { .. }
        ));
        if cfg.framing.end == [3] {
            assert_eq!(
                result(&cfg, b'0', FrameEnd::Block).block,
                BlockId::Intermediate(0)
            );
        }
    }
}
