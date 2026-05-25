use std::io::Cursor;

use num_enum::{IntoPrimitive, TryFromPrimitive};
use ptp_cursor::{PtpDeserialize, PtpSerialize, Read};
use ptp_macro::{PtpDeserialize, PtpSerialize};

#[derive(Debug, PartialEq, Eq, PtpSerialize, PtpDeserialize)]
struct Header {
    magic: u16,
    length: u32,
    flag: u8,
}

#[test]
fn named_struct_writes_fields_in_declaration_order() {
    let h = Header {
        magic: 0x1234,
        length: 0xDEAD_BEEF,
        flag: 0xAA,
    };
    let bytes = h.try_into_ptp().unwrap();
    assert_eq!(
        bytes,
        vec![
            0x34, 0x12, // magic
            0xEF, 0xBE, 0xAD, 0xDE, // length
            0xAA, // flag
        ],
    );
}

#[test]
fn named_struct_round_trips() {
    let h = Header {
        magic: 0x1234,
        length: 0xDEAD_BEEF,
        flag: 0xAA,
    };
    let bytes = h.try_into_ptp().unwrap();
    let parsed = Header::try_from_ptp(&bytes).unwrap();
    assert_eq!(parsed, h);
}

#[derive(Debug, PartialEq, Eq, PtpSerialize, PtpDeserialize)]
struct Point(i16, i16);

#[test]
fn tuple_struct_writes_in_field_order() {
    let p = Point(-1, 1);
    let bytes = p.try_into_ptp().unwrap();
    assert_eq!(
        bytes,
        vec![
            0xFF, 0xFF, // -1
            0x01, 0x00, // 1
        ],
    );
}

#[test]
fn tuple_struct_round_trips() {
    let p = Point(-1234, 5678);
    let bytes = p.try_into_ptp().unwrap();
    assert_eq!(Point::try_from_ptp(&bytes).unwrap(), p);
}

#[derive(Debug, PartialEq, Eq, PtpSerialize, PtpDeserialize)]
struct Marker;

#[test]
fn unit_struct_writes_zero_bytes() {
    let bytes = Marker.try_into_ptp().unwrap();
    assert!(bytes.is_empty());
}

#[test]
fn unit_struct_round_trips_empty_input() {
    let bytes = Marker.try_into_ptp().unwrap();
    assert_eq!(Marker::try_from_ptp(&bytes).unwrap(), Marker);
}

#[test]
fn unit_struct_rejects_non_empty_input() {
    let err = Marker::try_from_ptp(&[0x42]).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::UnexpectedEof);
}

#[repr(u16)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpSerialize, PtpDeserialize,
)]
enum Color {
    Red = 0x0001,
    Green = 0x0002,
    Blue = 0x0003,
}

#[test]
fn u16_enum_serializes_as_discriminant_le() {
    let bytes = Color::Green.try_into_ptp().unwrap();
    assert_eq!(bytes, vec![0x02, 0x00]);
}

#[test]
fn u16_enum_round_trips() {
    let bytes = Color::Blue.try_into_ptp().unwrap();
    assert_eq!(Color::try_from_ptp(&bytes).unwrap(), Color::Blue);
}

#[test]
fn u16_enum_rejects_unknown_discriminant() {
    let err = Color::try_from_ptp(&[0x99, 0x00]).unwrap_err();
    assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    assert!(
        err.to_string().contains("Color"),
        "error should mention the enum name: {err}",
    );
}

#[repr(i16)]
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, IntoPrimitive, TryFromPrimitive, PtpSerialize, PtpDeserialize,
)]
enum Trim {
    Negative = -1,
    Zero = 0,
    Positive = 1,
}

#[test]
fn i16_enum_round_trips_negative() {
    let bytes = Trim::Negative.try_into_ptp().unwrap();
    assert_eq!(bytes, vec![0xFF, 0xFF]);
    assert_eq!(Trim::try_from_ptp(&bytes).unwrap(), Trim::Negative);
}

#[derive(Debug, PartialEq, Eq, PtpSerialize, PtpDeserialize)]
struct Pixel {
    position: Point,
    color: Color,
}

#[test]
fn nested_derives_compose() {
    let p = Pixel {
        position: Point(3, -3),
        color: Color::Red,
    };
    let bytes = p.try_into_ptp().unwrap();
    assert_eq!(
        bytes,
        vec![
            0x03, 0x00, // x = 3
            0xFD, 0xFF, // y = -3
            0x01, 0x00, // Color::Red
        ],
    );
    assert_eq!(Pixel::try_from_ptp(&bytes).unwrap(), p);
}

#[test]
fn cursor_chained_reads_consume_only_their_own_bytes() {
    let buf = vec![
        0x34, 0x12, 0xEF, 0xBE, 0xAD, 0xDE, 0xAA, // Header
        0x05, 0x00, 0x07, 0x00, // Point(5, 7)
    ];
    let mut cur = Cursor::new(&buf[..]);

    let header = Header::try_read_ptp(&mut cur).unwrap();
    let point = Point::try_read_ptp(&mut cur).unwrap();

    assert_eq!(
        header,
        Header {
            magic: 0x1234,
            length: 0xDEAD_BEEF,
            flag: 0xAA,
        },
    );
    assert_eq!(point, Point(5, 7));
    cur.expect_end().unwrap();
}
