use serde::{Deserialize, Serialize};

/// Preset de ecualización con nombre, preamplificador y bandas para 20/31 bandas.
///
/// Puede almacenar ganancias para ambos modos (20 y 31 bandas) y realiza
/// conversión por interpolación logarítmica entre ellos.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EqPreset {
    pub name: String,
    pub preamp_gain: f32, // en dB, como en la UI (-9 a +9), no ganancia lineal.
    pub bands_20: Option<Vec<f32>>,
    pub bands_31: Option<Vec<f32>>,
}

impl EqPreset {
    /// Crea un nuevo preset de ecualización.
    pub fn new(
        name: &str,
        preamp_gain: f32,
        bands_20: Option<Vec<f32>>,
        bands_31: Option<Vec<f32>>,
    ) -> Self {
        Self {
            name: name.to_string(),
            preamp_gain,
            bands_20,
            bands_31,
        }
    }

    /// Devuelve las ganancias para 20 bandas, convirtiendo desde 31 si es necesario.
    pub fn get_gains_20(&self) -> Vec<f32> {
        if let Some(ref b) = self.bands_20 {
            b.clone()
        } else if let Some(ref b) = self.bands_31 {
            Self::convert_31_to_20(b)
        } else {
            vec![0.0; 20]
        }
    }

    /// Devuelve las ganancias para 31 bandas, convirtiendo desde 20 si es necesario.
    pub fn get_gains_31(&self) -> Vec<f32> {
        if let Some(ref b) = self.bands_31 {
            b.clone()
        } else if let Some(ref b) = self.bands_20 {
            Self::convert_20_to_31(b)
        } else {
            vec![0.0; 31]
        }
    }

    /// Convierte ganancias de 31 bandas a 20 bandas por interpolación logarítmica.
    pub fn convert_31_to_20(gains_31: &[f32]) -> Vec<f32> {
        let f20 = [
            22.4, 31.5, 45.0, 63.0, 90.0, 125.0, 180.0, 250.0, 355.0, 500.0, 710.0, 1000.0, 1400.0,
            2000.0, 2800.0, 4000.0, 5600.0, 8000.0, 11200.0, 16000.0,
        ];
        let f31 = [
            20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0,
            400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0,
            5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
        ];
        f20.iter()
            .map(|&f| Self::log_interpolate(f, &f31, gains_31))
            .collect()
    }

    /// Convierte ganancias de 20 bandas a 31 bandas por interpolación logarítmica.
    pub fn convert_20_to_31(gains_20: &[f32]) -> Vec<f32> {
        let f20 = [
            22.4, 31.5, 45.0, 63.0, 90.0, 125.0, 180.0, 250.0, 355.0, 500.0, 710.0, 1000.0, 1400.0,
            2000.0, 2800.0, 4000.0, 5600.0, 8000.0, 11200.0, 16000.0,
        ];
        let f31 = [
            20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0,
            400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0,
            5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
        ];
        f31.iter()
            .map(|&f| Self::log_interpolate(f, &f20, gains_20))
            .collect()
    }

    fn log_interpolate(target_f: f32, source_freqs: &[f32], source_gains: &[f32]) -> f32 {
        if source_freqs.is_empty() || source_gains.len() != source_freqs.len() {
            return 0.0;
        }
        if target_f <= source_freqs[0] {
            return source_gains[0];
        }
        let last = source_freqs.len() - 1;
        if target_f >= source_freqs[last] {
            return source_gains[last];
        }

        for i in 0..last {
            if target_f >= source_freqs[i] && target_f <= source_freqs[i + 1] {
                let f1 = source_freqs[i];
                let f2 = source_freqs[i + 1];
                let v1 = source_gains[i];
                let v2 = source_gains[i + 1];

                let t = (target_f.log10() - f1.log10()) / (f2.log10() - f1.log10());
                let val = v1 + t * (v2 - v1);
                return (val * 10.0).round() / 10.0;
            }
        }
        0.0
    }

    /// Devuelve una lista de presets predefinidos (Default, Ballad, Classical, etc.).
    pub fn default_presets() -> Vec<Self> {
        vec![
            // Default
            Self::new("Default", 0.0, Some(vec![0.0; 20]), Some(vec![0.0; 31])),
            // Ballad — Vocal-forward warmth with clarity
            Self::new(
                "Ballad",
                0.0,
                Some(vec![
                    2.0, 2.0, 1.5, 1.5, 1.5, 2.0, 2.5, 2.5, 2.5, 2.0, 2.0, 2.5, 2.5, 2.0, 1.5, 1.0,
                    1.0, 1.5, 2.0, 2.0,
                ]),
                None,
            ),
            // Classical — Natural dynamics with extended air
            Self::new(
                "Classical",
                0.0,
                Some(vec![
                    2.5, 2.5, 2.0, 2.0, 1.5, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.5, 2.0,
                    2.5, 3.0, 3.5, 3.5,
                ]),
                None,
            ),
            // Cumbia
            Self::new(
                "Cumbia",
                0.0,
                Some(vec![
                    3.5, 3.5, 3.0, 2.5, 1.5, 0.0, -1.0, -1.5, -1.0, 0.0, 1.5, 2.5, 3.0, 3.0, 2.5,
                    2.0, 2.5, 3.0, 3.5, 4.0,
                ]),
                None,
            ),
            // Electronic — Powerful low end, sparkling highs for EDM
            Self::new(
                "Electronic",
                0.0,
                Some(vec![
                    4.5, 5.0, 5.0, 4.5, 3.0, 1.0, -1.0, -2.0, -2.5, -2.5, -2.0, -1.0, 0.5, 1.5, 2.5,
                    3.5, 4.5, 5.0, 5.0, 4.5,
                ]),
                None,
            ),
            // Funk
            Self::new(
                "Funk",
                0.0,
                Some(vec![
                    3.0, 3.0, 3.5, 4.0, 3.0, 1.5, 0.0, -1.0, -1.5, -1.0, 0.0, 1.0, 2.0, 2.5, 2.0,
                    1.5, 1.0, 1.0, 1.5, 2.0,
                ]),
                None,
            ),
            // Glam Rock
            Self::new(
                "Glam Rock",
                0.0,
                Some(vec![
                    2.0, 2.5, 2.5, 2.0, 1.5, 1.0, 0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 3.0,
                    2.5, 2.0, 1.5, 1.0,
                ]),
                None,
            ),
            // Hard Rock
            Self::new(
                "Hard Rock",
                0.0,
                Some(vec![
                    3.5, 3.5, 3.0, 2.5, 1.5, 0.5, -1.0, -1.5, -2.0, -1.0, 0.0, 1.0, 2.0, 2.5, 3.0,
                    3.0, 3.5, 3.5, 4.0, 4.5,
                ]),
                None,
            ),
            // Heavy Metal
            Self::new(
                "Heavy Metal",
                0.0,
                Some(vec![
                    4.0, 4.0, 3.5, 3.0, 2.0, 0.5, -1.5, -2.0, -2.5, -2.0, -1.0, 0.0, 1.5, 2.5, 3.0,
                    3.5, 4.0, 4.0, 4.5, 5.0,
                ]),
                None,
            ),
            // Hip-Hop — Deep bass, crisp clarity, modern sound
            Self::new(
                "Hip-Hop",
                0.0,
                Some(vec![
                    5.0, 5.5, 5.0, 4.5, 3.0, 1.0, 0.0, -1.0, -1.5, -1.5, -1.0, 0.0, 1.0, 1.5, 2.0,
                    2.5, 3.0, 3.5, 4.0, 4.5,
                ]),
                None,
            ),
            // Jazz
            Self::new(
                "Jazz",
                0.0,
                Some(vec![
                    2.0, 2.0, 1.5, 1.5, 1.0, 0.5, 0.0, -0.5, -1.0, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5,
                    2.0, 2.5, 2.5, 3.0, 3.0,
                ]),
                None,
            ),
            // New Wave
            Self::new(
                "New Wave",
                0.0,
                Some(vec![
                    2.0, 2.0, 1.5, 1.0, 0.0, -1.0, -1.5, -1.0, 0.0, 1.0, 2.0, 2.5, 3.0, 3.5, 4.0,
                    3.5, 3.0, 2.5, 2.0, 1.5,
                ]),
                None,
            ),
            // Opera
            Self::new(
                "Opera",
                0.0,
                Some(vec![
                    -1.0, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.5, 4.0, 3.5,
                    2.5, 1.5, 1.0, 0.5, 0.0,
                ]),
                None,
            ),
            // Party — Loudness-optimized for crowded rooms
            Self::new(
                "Party",
                0.0,
                Some(vec![
                    5.5, 5.5, 5.0, 4.5, 3.5, 1.5, 0.0, -0.5, -0.5, 0.0, 0.0, 0.0, 0.5, 1.5, 2.5, 3.5,
                    4.5, 5.0, 5.5, 5.5,
                ]),
                None,
            ),
            // Pop
            Self::new(
                "Pop",
                0.0,
                Some(vec![
                    -1.5, -1.0, 0.0, 1.5, 2.5, 3.0, 2.5, 1.5, 0.0, -1.0, -2.0, -2.0, -1.5, -0.5,
                    0.5, 1.5, 2.0, 2.5, 2.0, 1.5,
                ]),
                None,
            ),
            // Post-Punk
            Self::new(
                "Post-Punk",
                0.0,
                Some(vec![
                    3.0, 3.0, 2.5, 1.5, 0.0, -1.0, -1.5, -1.0, 0.0, 1.0, 2.5, 3.5, 4.0, 4.5, 4.0,
                    3.5, 3.0, 2.5, 2.0, 1.5,
                ]),
                None,
            ),
            // Progressive Rock
            Self::new(
                "Progressive Rock",
                0.0,
                Some(vec![
                    2.5, 2.5, 2.0, 1.5, 1.0, 0.5, 0.0, -0.5, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5,
                    3.0, 3.5, 4.0, 4.0, 4.5,
                ]),
                None,
            ),
            // Punk Rock
            Self::new(
                "Punk Rock",
                0.0,
                Some(vec![
                    2.0, 2.0, 2.5, 3.0, 2.0, 1.0, 0.0, -1.0, -1.5, -1.0, 0.0, 1.0, 2.0, 2.5, 3.0,
                    3.0, 3.5, 3.5, 4.0, 4.5,
                ]),
                None,
            ),
            // Ranchera
            Self::new(
                "Ranchera",
                0.0,
                Some(vec![
                    1.0, 1.0, 1.5, 2.0, 2.5, 3.0, 3.0, 2.5, 2.0, 1.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5,
                    3.5, 4.0, 4.0, 4.5,
                ]),
                None,
            ),
            // Reggae — Deep sub-bass foundation, smooth mids
            Self::new(
                "Reggae",
                0.0,
                Some(vec![
                    5.0, 5.5, 5.0, 4.0, 2.5, 1.0, -0.5, -1.5, -1.0, 0.0, 0.5, 1.0, 1.5, 1.5, 1.0, 1.0,
                    0.5, 1.0, 1.5, 2.0,
                ]),
                None,
            ),
            // Reggaeton — Massive sub-bass, crisp highs
            Self::new(
                "Reggaeton",
                0.0,
                Some(vec![
                    6.0, 6.0, 5.5, 5.0, 3.5, 1.5, 0.0, -1.0, -1.5, -1.0, 0.0, 0.5, 1.0, 1.5, 2.0,
                    2.5, 3.0, 3.5, 4.0, 4.5,
                ]),
                None,
            ),
            // Rock
            Self::new(
                "Rock",
                0.0,
                Some(vec![
                    3.0, 3.0, 2.5, 2.5, 2.0, 1.0, 0.0, -1.0, -1.5, -1.0, 0.0, 1.0, 1.5, 2.0, 2.5,
                    3.0, 3.5, 3.5, 4.0, 4.0,
                ]),
                None,
            ),
            // Rock And Roll
            Self::new(
                "Rock And Roll",
                0.0,
                Some(vec![
                    2.5, 2.5, 2.0, 2.0, 1.5, 0.5, -1.0, -1.5, -2.0, -1.5, -1.0, 0.0, 1.0, 2.0, 2.5,
                    3.0, 3.0, 3.5, 3.5, 4.0,
                ]),
                None,
            ),
            // Salsa
            Self::new(
                "Salsa",
                0.0,
                Some(vec![
                    2.5, 2.5, 2.5, 2.0, 1.5, 1.0, 0.5, 0.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.0,
                    2.5, 2.5, 2.0, 2.0,
                ]),
                None,
            ),
            // Soul — Smooth warmth with modern presence
            Self::new(
                "Soul",
                0.0,
                Some(vec![
                    2.5, 2.5, 2.0, 2.0, 1.5, 1.0, 0.5, 1.0, 1.5, 2.0, 2.0, 1.5, 1.0, 1.0, 1.5, 2.0,
                    2.5, 2.5, 3.0, 3.0,
                ]),
                None,
            ),
        ]
    }
}

impl std::fmt::Display for EqPreset {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}

/// Formato de archivo JSON para exportar/importar presets de ecualizador.
/// Siempre incluye ambos sets de bandas (20 y 31) en export,
/// pero puede aceptar uno solo en import con conversión automática.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EqPresetFile {
    pub version: u32,
    pub name: String,
    pub preamp_gain: f32,
    pub bands_20: Option<Vec<f32>>,
    pub bands_31: Option<Vec<f32>>,
}

impl From<&EqPreset> for EqPresetFile {
    fn from(preset: &EqPreset) -> Self {
        Self {
            version: 1,
            name: preset.name.clone(),
            preamp_gain: preset.preamp_gain,
            bands_20: Some(preset.get_gains_20()), // Always include both
            bands_31: Some(preset.get_gains_31()),
        }
    }
}

/// Serializa un preset a JSON string para exportar a archivo.
pub fn preset_to_json(preset: &EqPreset) -> Result<String, serde_json::Error> {
    let file: EqPresetFile = preset.into();
    serde_json::to_string_pretty(&file)
}

/// Deserializa un preset desde JSON string, con conversión automática
/// si solo un set de bandas está presente.
pub fn preset_from_json(json: &str) -> Result<EqPreset, serde_json::Error> {
    let mut file: EqPresetFile = serde_json::from_str(json)?;
    // If only one band set is present, convert from the other
    if file.bands_20.is_none() && file.bands_31.is_some() {
        let gains_20 = EqPreset::convert_31_to_20(file.bands_31.as_ref().unwrap());
        file.bands_20 = Some(gains_20);
    } else if file.bands_31.is_none() && file.bands_20.is_some() {
        let gains_31 = EqPreset::convert_20_to_31(file.bands_20.as_ref().unwrap());
        file.bands_31 = Some(gains_31);
    }
    Ok(EqPreset::new(
        &file.name,
        file.preamp_gain,
        file.bands_20,
        file.bands_31,
    ))
}

#[cfg(test)]
mod preset_io_tests {
    use super::*;

    #[test]
    fn test_preset_to_json_round_trip() {
        let preset = EqPreset::new(
            "Test",
            1.5,
            Some(vec![
                -3.0, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 2.0, 1.0, 0.0, -1.0, -2.0, -3.0, -2.0,
                -1.0, 0.0, 1.0, 2.0, 3.0, 4.0,
            ]),
            None, // Only 20-band set — conversion will happen on import
        );

        let json = preset_to_json(&preset).unwrap();
        let parsed = preset_from_json(&json).unwrap();

        assert_eq!(parsed.name, "Test");
        assert!((parsed.preamp_gain - 1.5).abs() < 0.001);
        // Both band sets should be present after serialization
        assert!(parsed.bands_20.is_some());
        assert!(parsed.bands_31.is_some());
        // bands_20 should match exactly
        let original_20 = preset.get_gains_20();
        let parsed_20 = parsed.get_gains_20();
        assert_eq!(original_20.len(), parsed_20.len());
        for (a, b) in original_20.iter().zip(parsed_20.iter()) {
            assert!((a - b).abs() < 0.001);
        }
    }

    #[test]
    fn test_import_single_band_set_conversion() {
        // Import a file with only bands_20
        let json = serde_json::json!({
            "version": 1,
            "name": "Converted",
            "preamp_gain": 0.0,
            "bands_20": [1.0, 1.5, 2.0, 2.5, 3.0, 2.5, 2.0, 1.5, 1.0, 0.5,
                         0.0, -0.5, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5],
            "bands_31": null
        })
        .to_string();

        let preset = preset_from_json(&json).unwrap();
        assert_eq!(preset.name, "Converted");
        assert!(preset.bands_20.is_some());
        assert!(preset.bands_31.is_some()); // Should be auto-generated
        assert_eq!(preset.bands_20.as_ref().unwrap().len(), 20);
        assert_eq!(preset.bands_31.as_ref().unwrap().len(), 31);
    }

    #[test]
    fn test_import_with_both_bands() {
        let json = serde_json::json!({
            "version": 1,
            "name": "Full Preset",
            "preamp_gain": -2.0,
            "bands_20": vec![1.0; 20],
            "bands_31": vec![2.0; 31]
        })
        .to_string();

        let preset = preset_from_json(&json).unwrap();
        assert_eq!(preset.name, "Full Preset");
        assert_eq!(preset.bands_20.as_ref().unwrap().len(), 20);
        assert_eq!(preset.bands_31.as_ref().unwrap().len(), 31);
        assert!((preset.bands_20.as_ref().unwrap()[0] - 1.0).abs() < 0.001);
        assert!((preset.bands_31.as_ref().unwrap()[0] - 2.0).abs() < 0.001);
    }

    #[test]
    fn test_json_contains_both_band_sets_on_export() {
        let preset = EqPreset::new("ExportTest", 0.0, Some(vec![0.5; 20]), None);
        let json = preset_to_json(&preset).unwrap();
        let parsed: EqPresetFile = serde_json::from_str(&json).unwrap();

        // Both should be present in the JSON file
        assert!(parsed.bands_20.is_some());
        assert!(parsed.bands_31.is_some()); // Auto-converted on export
        assert_eq!(parsed.version, 1);
    }
}
