use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqPreset {
    pub name: String,
    pub preamp_gain: f32, // en dB, como en la UI (-9 a +9), no ganancia lineal.
    pub bands_20: Option<Vec<f32>>,
    pub bands_31: Option<Vec<f32>>,
}

impl EqPreset {
    pub fn new(name: &str, preamp_gain: f32, bands_20: Option<Vec<f32>>, bands_31: Option<Vec<f32>>) -> Self {
        Self {
            name: name.to_string(),
            preamp_gain,
            bands_20,
            bands_31,
        }
    }

    pub fn get_gains_20(&self) -> Vec<f32> {
        if let Some(ref b) = self.bands_20 {
            b.clone()
        } else if let Some(ref b) = self.bands_31 {
            Self::convert_31_to_20(b)
        } else {
            vec![0.0; 20]
        }
    }

    pub fn get_gains_31(&self) -> Vec<f32> {
        if let Some(ref b) = self.bands_31 {
            b.clone()
        } else if let Some(ref b) = self.bands_20 {
            Self::convert_20_to_31(b)
        } else {
            vec![0.0; 31]
        }
    }

    pub fn convert_31_to_20(gains_31: &[f32]) -> Vec<f32> {
        let f20 = [
            22.4, 31.5, 45.0, 63.0, 90.0, 125.0, 180.0, 250.0, 355.0, 500.0, 
            710.0, 1000.0, 1400.0, 2000.0, 2800.0, 4000.0, 5600.0, 8000.0, 11200.0, 16000.0
        ];
        let f31 = [
            20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 
            200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 
            2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0
        ];
        f20.iter().map(|&f| Self::log_interpolate(f, &f31, gains_31)).collect()
    }

    pub fn convert_20_to_31(gains_20: &[f32]) -> Vec<f32> {
        let f20 = [
            22.4, 31.5, 45.0, 63.0, 90.0, 125.0, 180.0, 250.0, 355.0, 500.0, 
            710.0, 1000.0, 1400.0, 2000.0, 2800.0, 4000.0, 5600.0, 8000.0, 11200.0, 16000.0
        ];
        let f31 = [
            20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 
            200.0, 250.0, 315.0, 400.0, 500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 
            2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0, 8000.0, 10000.0, 12500.0, 16000.0, 20000.0
        ];
        f31.iter().map(|&f| Self::log_interpolate(f, &f20, gains_20)).collect()
    }

    fn log_interpolate(target_f: f32, source_freqs: &[f32], source_gains: &[f32]) -> f32 {
        if source_freqs.is_empty() || source_gains.len() != source_freqs.len() { return 0.0; }
        if target_f <= source_freqs[0] { return source_gains[0]; }
        let last = source_freqs.len() - 1;
        if target_f >= source_freqs[last] { return source_gains[last]; }
        
        for i in 0..last {
            if target_f >= source_freqs[i] && target_f <= source_freqs[i+1] {
                let f1 = source_freqs[i];
                let f2 = source_freqs[i+1];
                let v1 = source_gains[i];
                let v2 = source_gains[i+1];
                
                let t = (target_f.log10() - f1.log10()) / (f2.log10() - f1.log10());
                let val = v1 + t * (v2 - v1);
                return (val * 10.0).round() / 10.0;
            }
        }
        0.0
    }

    pub fn default_presets() -> Vec<Self> {
        vec![
            // Default
            Self::new("Default", 0.0, Some(vec![0.0; 20]), Some(vec![0.0; 31])),
            
            // Pop
            Self::new("Pop", 0.0, 
                Some(vec![-1.5, -1.0, 0.0, 1.5, 2.5, 3.0, 2.5, 1.5, 0.0, -1.0, -2.0, -2.0, -1.5, -0.5, 0.5, 1.5, 2.0, 2.5, 2.0, 1.5]), None),
            
            // Party
            Self::new("Party", 2.0, 
                Some(vec![5.0, 5.0, 5.0, 4.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 5.0, 5.0]), None),
                
            // Electronic
            Self::new("Electronic", 1.0, 
                Some(vec![4.0, 4.5, 4.5, 4.0, 3.0, 1.0, -1.0, -2.0, -2.5, -2.5, -2.0, -1.0, 0.0, 1.0, 2.0, 3.0, 4.0, 4.0, 4.5, 4.0]), None),
                
            // Rock And Roll
            Self::new("Rock And Roll", 0.0,
                Some(vec![2.5, 2.5, 2.0, 2.0, 1.5, 0.5, -1.0, -1.5, -2.0, -1.5, -1.0, 0.0, 1.0, 2.0, 2.5, 3.0, 3.0, 3.5, 3.5, 4.0]), None),
                
            // Hard Rock
            Self::new("Hard Rock", 0.0,
                Some(vec![3.5, 3.5, 3.0, 2.5, 1.5, 0.5, -1.0, -1.5, -2.0, -1.0, 0.0, 1.0, 2.0, 2.5, 3.0, 3.0, 3.5, 3.5, 4.0, 4.5]), None),
                
            // Heavy Metal
            Self::new("Heavy Metal", 0.0,
                Some(vec![4.0, 4.0, 3.5, 3.0, 2.0, 0.5, -1.5, -2.0, -2.5, -2.0, -1.0, 0.0, 1.5, 2.5, 3.0, 3.5, 4.0, 4.0, 4.5, 5.0]), None),
                
            // Punk Rock
            Self::new("Punk Rock", 0.0,
                Some(vec![2.0, 2.0, 2.5, 3.0, 2.0, 1.0, 0.0, -1.0, -1.5, -1.0, 0.0, 1.0, 2.0, 2.5, 3.0, 3.0, 3.5, 3.5, 4.0, 4.5]), None),
                
            // Jazz
            Self::new("Jazz", 0.0,
                Some(vec![2.0, 2.0, 1.5, 1.5, 1.0, 0.5, 0.0, -0.5, -1.0, -1.0, -0.5, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 2.5, 3.0, 3.0]), None),
                
            // Funk
            Self::new("Funk", 0.0,
                Some(vec![3.0, 3.0, 3.5, 4.0, 3.0, 1.5, 0.0, -1.0, -1.5, -1.0, 0.0, 1.0, 2.0, 2.5, 2.0, 1.5, 1.0, 1.0, 1.5, 2.0]), None),
                
            // Classical
            Self::new("Classical", 0.0,
                Some(vec![3.0, 3.0, 2.5, 2.5, 2.0, 1.5, 1.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.0]), None),
                
            // Ballad
            Self::new("Ballad", 0.0,
                Some(vec![1.5, 1.5, 1.0, 1.0, 1.5, 2.0, 2.5, 3.0, 3.0, 2.5, 2.0, 1.5, 1.0, 0.5, 0.0, 0.0, 0.5, 1.0, 1.5, 2.0]), None),
                
            // Mariachi
            Self::new("Mariachi", 0.0,
                Some(vec![1.0, 1.0, 1.5, 2.0, 2.5, 3.0, 3.0, 2.5, 2.0, 1.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 3.5, 4.0, 4.0, 4.5]), None),
                
            // Salsa
            Self::new("Salsa", 0.0,
                Some(vec![2.5, 2.5, 2.5, 2.0, 1.5, 1.0, 0.5, 0.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.0, 2.5, 2.5, 2.0, 2.0]), None),
                
            // Hip-Hop
            Self::new("Hip-Hop", 1.0,
                Some(vec![5.0, 5.0, 4.5, 4.0, 3.0, 1.0, 0.0, -1.0, -1.5, -1.5, -1.0, 0.0, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.0]), None),
                
            // Reggaeton
            Self::new("Reggaeton", 1.0,
                Some(vec![5.5, 5.5, 5.0, 4.5, 3.5, 1.5, 0.0, -1.0, -1.5, -1.0, 0.0, 0.5, 1.0, 1.5, 2.0, 2.5, 3.0, 3.5, 4.0, 4.0]), None),
        ]
    }
}
