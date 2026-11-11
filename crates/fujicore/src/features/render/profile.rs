use std::io;

use ptp_cursor::{ExactString, PtpDeserialize, PtpSerialize, Read};

/// Bytes the profile code occupies on the wire, including its padding.
const CODE_REGION_LEN: usize = 0x1FF;

/// The fields of a camera's conversion profile, in wire order.
pub trait ConversionProfileFields: Sized {
    const FIELD_COUNT: i16;

    fn write_fields(&self, buf: &mut Vec<u8>) -> io::Result<()>;

    fn read_fields(values: &[i32]) -> io::Result<Self>;
}

pub fn profile_value(values: &[i32], position: usize, id: &str) -> io::Result<i32> {
    values.get(position).copied().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "profile ends at position {}, before `{id}` at position {position}",
                values.len()
            ),
        )
    })
}

/// A conversion profile as exchanged with the camera.
#[derive(Debug, Clone)]
pub struct ConversionProfile<T> {
    pub code: ExactString,
    pub fields: T,
}

impl<T: ConversionProfileFields> PtpSerialize for ConversionProfile<T> {
    fn try_into_ptp(&self) -> io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        self.try_write_ptp(&mut buf)?;
        Ok(buf)
    }

    fn try_write_ptp(&self, buf: &mut Vec<u8>) -> io::Result<()> {
        T::FIELD_COUNT.try_write_ptp(buf)?;

        let start = buf.len();
        self.code.try_write_ptp(buf)?;
        let padding = CODE_REGION_LEN
            .checked_sub(buf.len() - start)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("profile code `{}` is too long", self.code.as_str()),
                )
            })?;
        buf.resize(buf.len() + padding, 0);

        self.fields.write_fields(buf)
    }
}

impl<T: ConversionProfileFields> PtpDeserialize for ConversionProfile<T> {
    fn try_from_ptp(buf: &[u8]) -> io::Result<Self> {
        let cur = &mut io::Cursor::new(buf);
        let _count = i16::try_read_ptp(cur)?;

        let code = ExactString::try_read_ptp(cur)?;
        let code_len = code.encode_utf16().count() * 2 + 1;
        let padding = CODE_REGION_LEN.checked_sub(code_len).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("profile code `{}` is too long", code.as_str()),
            )
        })?;
        io::Read::read_exact(cur, &mut vec![0; padding])?;

        let mut rest = Vec::new();
        io::Read::read_to_end(cur, &mut rest)?;
        let (chunks, remainder) = rest.as_chunks::<4>();
        if !remainder.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("profile ends {} bytes into a field", remainder.len()),
            ));
        }
        let values: Vec<i32> = chunks.iter().copied().map(i32::from_le_bytes).collect();

        let fields = T::read_fields(&values)?;
        Ok(Self { code, fields })
    }

    /// Reads to the end of the input.
    fn try_read_ptp<R: Read>(cur: &mut R) -> io::Result<Self> {
        let mut buf = Vec::new();
        io::Read::read_to_end(cur, &mut buf)?;
        Self::try_from_ptp(&buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Two(i32, i32);

    impl ConversionProfileFields for Two {
        const FIELD_COUNT: i16 = 2;

        fn write_fields(&self, buf: &mut Vec<u8>) -> io::Result<()> {
            self.0.try_write_ptp(buf)?;
            self.1.try_write_ptp(buf)
        }

        fn read_fields(values: &[i32]) -> io::Result<Self> {
            Ok(Self(
                profile_value(values, 0, "a")?,
                profile_value(values, 1, "b")?,
            ))
        }
    }

    fn two_bytes() -> Vec<u8> {
        ConversionProfile {
            code: ExactString::from(String::from("FF179502")),
            fields: Two(1, -2),
        }
        .try_into_ptp()
        .unwrap()
    }

    #[test]
    fn round_trips_and_keeps_the_code() {
        let profile = ConversionProfile {
            code: ExactString::from(String::from("FF179502")),
            fields: Two(1, -2),
        };
        let bytes = profile.try_into_ptp().unwrap();
        assert_eq!(bytes.len(), 2 + CODE_REGION_LEN + 8);

        let read = ConversionProfile::<Two>::try_from_ptp(&bytes).unwrap();
        assert_eq!(read.code.as_str(), "FF179502");
        assert_eq!(read.fields, Two(1, -2));
    }

    #[test]
    fn ignores_the_field_count() {
        let mut bytes = two_bytes();
        bytes[0] = 3;
        let read = ConversionProfile::<Two>::try_from_ptp(&bytes).unwrap();
        assert_eq!(read.fields, Two(1, -2));
    }

    #[test]
    fn ignores_values_past_the_layout() {
        let mut bytes = two_bytes();
        bytes.extend_from_slice(&7i32.to_le_bytes());
        let read = ConversionProfile::<Two>::try_from_ptp(&bytes).unwrap();
        assert_eq!(read.fields, Two(1, -2));
    }

    #[test]
    fn rejects_a_profile_that_ends_before_a_field() {
        let mut bytes = two_bytes();
        bytes.truncate(bytes.len() - 4);
        let err = ConversionProfile::<Two>::try_from_ptp(&bytes).unwrap_err();
        assert_eq!(
            err.to_string(),
            "profile ends at position 1, before `b` at position 1"
        );
    }

    #[test]
    fn rejects_a_profile_that_ends_inside_a_field() {
        let mut bytes = two_bytes();
        bytes.truncate(bytes.len() - 2);
        assert!(ConversionProfile::<Two>::try_from_ptp(&bytes).is_err());
    }

    mod x_t5 {
        use super::*;
        use crate::generated::{options::*, renders::XT5RenderProfile};

        const LAYOUT_LEN: usize = 29;

        /// As the camera reported it for a RAF shot in Provia with As Shot white balance.
        fn as_shot() -> ConversionProfile<XT5RenderProfile> {
            ConversionProfile {
                code: ExactString::from(String::from("FF179502")),
                fields: XT5RenderProfile {
                    file_type: FileType::Jpeg,
                    image_size: ImageSize::X7728x5152,
                    image_quality: ImageQuality::Fine,
                    exposure_offset: ExposureOffset::Zero,
                    dynamic_range: Some(DynamicRange::Hdr400),
                    dynamic_range_priority: DynamicRangePriority::Off,
                    film_simulation: FilmSimulation::Provia,
                    grain_effect: GrainEffect::Off,
                    color_chrome_effect: ColorChromeEffect::Off,
                    white_balance_as_shot: 1,
                    white_balance: WhiteBalance::AsShot,
                    white_balance_shift_red: None,
                    white_balance_shift_blue: None,
                    white_balance_temperature: None,
                    highlight_tone: Some(HighlightTone::default()),
                    shadow_tone: Some(ShadowTone::default()),
                    color: Some(Color::default()),
                    sharpness: Sharpness::default(),
                    noise_reduction: NoiseReduction::Zero,
                    lens_modulation_optimizer: LensModulationOptimizer::On,
                    color_space: ColorSpace::Srgb,
                    monochromatic_color_temperature: None,
                    smooth_skin_effect: SmoothSkinEffect::Off,
                    color_chrome_fx_blue: ColorChromeFxBlue::Off,
                    monochromatic_color_tint: None,
                    clarity: Clarity::default(),
                    teleconverter: Teleconverter::Off,
                },
            }
        }

        fn values(bytes: &[u8]) -> Vec<i32> {
            bytes[2 + CODE_REGION_LEN..]
                .as_chunks::<4>()
                .0
                .iter()
                .copied()
                .map(i32::from_le_bytes)
                .collect()
        }

        #[test]
        fn writes_every_field_with_unused_ones_zeroed() {
            let bytes = as_shot().try_into_ptp().unwrap();
            let values = values(&bytes);
            assert_eq!(values.len(), LAYOUT_LEN);
            assert_eq!(values[0], 0, "shoot_condition");
            assert_eq!(values[LAYOUT_LEN - 1], 0, "portrait_enhancer");
        }

        #[test]
        fn reads_a_reply_without_the_fields_it_does_not_use() {
            let profile = as_shot();
            let bytes = profile.try_into_ptp().unwrap();
            let reply = &bytes[..bytes.len() - 4];
            assert_eq!(reply.len(), 625);

            let read = ConversionProfile::<XT5RenderProfile>::try_from_ptp(reply).unwrap();
            assert_eq!(read.code.as_str(), "FF179502");
            assert_eq!(
                format!("{:?}", read.fields),
                format!("{:?}", profile.fields)
            );
        }

        #[test]
        fn rejects_a_reply_without_a_field_it_uses() {
            let bytes = as_shot().try_into_ptp().unwrap();
            let reply = &bytes[..bytes.len() - 8];
            let err = ConversionProfile::<XT5RenderProfile>::try_from_ptp(reply).unwrap_err();
            assert!(err.to_string().contains("`teleconverter`"), "{err}");
        }
    }
}
