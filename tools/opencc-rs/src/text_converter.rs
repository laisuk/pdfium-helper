//! Configured CLI text pipeline: normalization, conversion, then optional DeTofu.
use opencc_fmmseg::{DetofuMap, OpenCC};
use opencc_utils::normalize_with;
pub use opencc_utils::NormalizationMode;

pub struct TextConverter<F> {
    convert: F,
}

impl<F: Fn(&str) -> String> TextConverter<F> {
    pub fn new(convert: F) -> Self {
        Self { convert }
    }
    pub fn convert(&self, text: &str) -> String {
        (self.convert)(text)
    }
}

pub struct TextConverterOptions<'a> {
    pub config: &'a str,
    pub punctuation: bool,
    pub normalization: NormalizationMode,
    pub detofu_map: Option<&'a DetofuMap>,
}

pub fn create_text_converter<'a>(
    engine: &'a OpenCC,
    options: TextConverterOptions<'a>,
) -> TextConverter<impl Fn(&str) -> String + 'a> {
    TextConverter::new(move |text: &str| {
        let normalized = normalize_with(
            text,
            options.normalization,
            |text| engine.normalize_compat(text),
            |text| engine.normalize_compat_extended(text),
        );
        let converted = engine.convert(normalized.as_ref(), options.config, options.punctuation);
        match options.detofu_map {
            Some(map) => map.detofu(&converted),
            None => converted,
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pipeline_normalizes_then_converts_then_applies_optional_detofu() {
        let engine = OpenCC::new();
        let map = DetofuMap::builtin(opencc_fmmseg::DetofuLevel::ExtB);
        for normalization in [
            NormalizationMode::None,
            NormalizationMode::Compat,
            NormalizationMode::CompatExtended,
        ] {
            for enabled in [false, true] {
                let converter = create_text_converter(
                    &engine,
                    TextConverterOptions {
                        config: "t2s",
                        punctuation: true,
                        normalization,
                        detofu_map: if enabled { Some(&map) } else { None },
                    },
                );
                let input = "Ａ車「漢語」𬴂";
                let normalized = normalize_with(
                    input,
                    normalization,
                    |s| engine.normalize_compat(s),
                    |s| engine.normalize_compat_extended(s),
                );
                let converted = engine.convert(&normalized, "t2s", true);
                let expected = if enabled {
                    map.detofu(&converted)
                } else {
                    converted
                };
                assert_eq!(converter.convert(input), expected);
            }
        }
    }
    #[test]
    fn detofu_runs_after_conversion() {
        let engine = OpenCC::new();
        let map = DetofuMap::builtin(opencc_fmmseg::DetofuLevel::ExtB);
        let converter = create_text_converter(
            &engine,
            TextConverterOptions {
                config: "t2s",
                punctuation: false,
                normalization: NormalizationMode::None,
                detofu_map: Some(&map),
            },
        );
        // Fallback produces traditional 騑; running conversion afterward would simplify it.
        assert_eq!(converter.convert("𬴂"), "騑");
    }

    #[test]
    fn custom_fallback_observes_normalized_and_converted_text() {
        let engine = OpenCC::new();
        let map =
            DetofuMap::builtin(opencc_fmmseg::DetofuLevel::ExtB).with_custom_pairs(&[('车', 'X')]);
        let converter = create_text_converter(
            &engine,
            TextConverterOptions {
                config: "t2s",
                punctuation: false,
                normalization: NormalizationMode::Compat,
                detofu_map: Some(&map),
            },
        );
        assert_eq!(converter.convert("車"), "X");
    }
}
