//! Configured CLI text pipeline: normalization, conversion, then optional DeTofu.
use opencc_jieba_rs::{DetofuLevel, OpenCC};
use opencc_utils::normalize_with;
pub use opencc_utils::NormalizationMode;

pub struct TextConverterJieba<F> {
    convert: F,
}

impl<F: Fn(&str) -> String> TextConverterJieba<F> {
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
    pub detofu: bool,
}

pub fn create_text_converter<'a>(
    engine: &'a OpenCC,
    options: TextConverterOptions<'a>,
) -> TextConverterJieba<impl Fn(&str) -> String + 'a> {
    TextConverterJieba::new(move |text: &str| {
        let normalized = normalize_with(
            text,
            options.normalization,
            |text| engine.normalize_compat(text),
            |text| engine.normalize_compat_extended(text),
        );
        let converted = engine.convert(normalized.as_ref(), options.config, options.punctuation);
        if options.detofu {
            engine.detofu(&converted, DetofuLevel::ExtB)
        } else {
            converted
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pipeline_normalizes_then_converts_then_applies_optional_detofu() {
        let engine = OpenCC::new();

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
                        detofu: enabled,
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
                    engine.detofu(&converted, DetofuLevel::ExtB)
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

        let converter = create_text_converter(
            &engine,
            TextConverterOptions {
                config: "t2s",
                punctuation: false,
                normalization: NormalizationMode::None,
                detofu: true,
            },
        );
        // Fallback produces traditional 騑; running conversion afterward would simplify it.
        assert_eq!(converter.convert("𬴂"), "騑");
    }
}
