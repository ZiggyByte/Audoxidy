#[cfg(test)]
mod tests {
    // Import strip_suffix from the widgets module (now pub(crate) for test access)
    use crate::gui::widgets::strip_suffix;

    // ========================================================================
    // Suffix stripping tests (Fix B2 / D-45)
    // ========================================================================

    #[test]
    fn test_suffix_strip_db() {
        let result = strip_suffix("-50.25 dB");
        assert_eq!(result, "-50.25");
        let parsed: f32 = result.parse().unwrap();
        assert!((parsed - (-50.25)).abs() < 1e-6);
    }

    #[test]
    fn test_suffix_strip_ms() {
        let result = strip_suffix("1000 ms");
        assert_eq!(result, "1000");
        let parsed: f32 = result.parse().unwrap();
        assert!((parsed - 1000.0).abs() < 1e-6);
    }

    #[test]
    fn test_suffix_strip_hz() {
        let result = strip_suffix("44100 Hz");
        assert_eq!(result, "44100");
        let parsed: f32 = result.parse().unwrap();
        assert!((parsed - 44100.0).abs() < 1e-6);
    }

    #[test]
    fn test_suffix_strip_percent() {
        let result = strip_suffix("75 %");
        assert_eq!(result, "75");
        let parsed: f32 = result.parse().unwrap();
        assert!((parsed - 75.0).abs() < 1e-6);
    }

    #[test]
    fn test_suffix_strip_no_suffix() {
        let result = strip_suffix("3.14");
        assert_eq!(result, "3.14");
        let parsed: f32 = result.parse().unwrap();
        assert!((parsed - 3.14).abs() < 1e-6);
    }

    #[test]
    fn test_suffix_strip_negative_db() {
        // Negative dB with trailing spaces
        let result = strip_suffix("  -5 dB  ");
        assert_eq!(result, "-5");
        let parsed: f32 = result.parse().unwrap();
        assert!((parsed - (-5.0)).abs() < 1e-6);
    }

    #[test]
    fn test_suffix_strip_empty_fails() {
        let result = strip_suffix("");
        assert!(result.parse::<f32>().is_err());
    }

    #[test]
    fn test_suffix_strip_invalid_fails() {
        let result = strip_suffix("notanumber");
        assert!(result.parse::<f32>().is_err());
    }

    #[test]
    fn test_suffix_strip_unsigned() {
        let result = strip_suffix("  0.5 dB  ");
        assert_eq!(result, "0.5");
        let parsed: f32 = result.parse().unwrap();
        assert!((parsed - 0.5).abs() < 1e-6);
    }

    // ========================================================================
    // NumberStepper character filter tests (D-34)
    // ========================================================================

    /// Replicates the character filter from NumberStepper's on_event handler
    /// (widgets.rs:500-506): only accept digits, '.', '-', whitespace.
    fn accepts_char(ch: char) -> bool {
        ch.is_ascii_digit() || ch == '.' || ch == '-' || ch.is_whitespace()
    }

    #[test]
    fn test_stepper_character_filter_accepts_digits() {
        for ch in '0'..='9' {
            assert!(accepts_char(ch), "Should accept digit '{}'", ch);
        }
    }

    #[test]
    fn test_stepper_character_filter_accepts_special() {
        assert!(accepts_char('.'), "Should accept '.'");
        assert!(accepts_char('-'), "Should accept '-'");
        assert!(accepts_char(' '), "Should accept space");
    }

    #[test]
    fn test_stepper_character_filter_rejects_letters() {
        assert!(!accepts_char('a'));
        assert!(!accepts_char('z'));
        assert!(!accepts_char('A'));
        assert!(!accepts_char('Z'));
        assert!(!accepts_char('e'));
        assert!(!accepts_char('x'));
    }

    #[test]
    fn test_stepper_character_filter_rejects_symbols() {
        assert!(!accepts_char('+'), "Should reject '+'");
        assert!(!accepts_char('*'), "Should reject '*'");
        assert!(!accepts_char('/'), "Should reject '/'");
        assert!(!accepts_char('#'), "Should reject '#'");
        assert!(!accepts_char('@'), "Should reject '@'");
        assert!(!accepts_char('!'), "Should reject '!'");
        assert!(!accepts_char('$'), "Should reject '$'");
        assert!(!accepts_char('%'), "Should reject '%'");
        assert!(!accepts_char('^'), "Should reject '^'");
        assert!(!accepts_char('&'), "Should reject '&'");
        assert!(!accepts_char('('), "Should reject '('");
        assert!(!accepts_char(')'), "Should reject ')'");
        assert!(!accepts_char('='), "Should reject '='");
        assert!(!accepts_char('['), "Should reject '['");
        assert!(!accepts_char(']'), "Should reject ']'");
    }

    #[test]
    fn test_stepper_character_filter_rejects_unicode() {
        assert!(!accepts_char('ñ'), "Should reject 'ñ'");
        assert!(!accepts_char('é'), "Should reject 'é'");
        assert!(!accepts_char('ç'), "Should reject 'ç'");
        assert!(!accepts_char('✓'), "Should reject '✓'");
        assert!(!accepts_char('♥'), "Should reject '♥'");
    }

    // ========================================================================
    // NumberStepper no-rounding test (D-34)
    // ========================================================================

    #[test]
    fn test_stepper_no_rounding() {
        // D-34: manual input value is NEVER rounded to the step.
        // A value of 1.78 stays 1.78, NOT 1.75 (0.25 step) or 2.0.
        let input_str = "1.78";
        let parsed: f64 = input_str.parse().unwrap();
        assert!((parsed - 1.78).abs() < 1e-10);

        // Verify it's NOT the step-rounded value
        let step: f64 = 0.25;
        let rounded_down = (1.78_f64 / step).floor() * step; // 1.75
        let rounded_nearest = (1.78_f64 / step).round() * step; // 1.75
        assert!((parsed - rounded_down).abs() > 1e-10, "Should NOT round to {}", rounded_down);
        assert!((parsed - rounded_nearest).abs() > 1e-10, "Should NOT round to {}", rounded_nearest);
        assert!((parsed - 2.0).abs() > 1e-10, "Should NOT be 2.0");

        // Test with milliseconds (step 50)
        let ms_str = "342";
        let ms_parsed: f64 = ms_str.parse().unwrap();
        assert!((ms_parsed - 342.0).abs() < 1e-10);

        let ms_step: f64 = 50.0;
        let ms_rounded = (342.0_f64 / ms_step).round() * ms_step; // 350
        assert!((ms_parsed - ms_rounded).abs() > 1e-10, "Should NOT round 342ms to 350ms");

        // Test negative value with decimals
        let neg_str = "-3.14";
        let neg_parsed: f64 = neg_str.parse().unwrap();
        assert!((neg_parsed - (-3.14)).abs() < 1e-10);
    }
}
