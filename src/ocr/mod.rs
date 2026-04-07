use image::DynamicImage;
use anyhow::Result;
use ocrs::{OcrEngine, OcrEngineParams, ImageSource};
use rten::Model;
use std::path::Path;

pub trait OcrProvider: Send + Sync {
    fn extract_text(&self, image: &DynamicImage) -> Result<String>;
}

pub struct LocalOcrProvider {
    engine: OcrEngine,
}

impl LocalOcrProvider {
    pub fn new<P: AsRef<Path>>(model_dir: P) -> Result<Self> {
        let detect_model_path = model_dir.as_ref().join("text-detection.rten");
        let rec_model_path = model_dir.as_ref().join("text-recognition.rten");

        if !detect_model_path.exists() || !rec_model_path.exists() {
            anyhow::bail!("OCR models (.rten) not found in {:?}. Please download them.", model_dir.as_ref());
        }

        let detect_model = Model::load_file(detect_model_path)?;
        let rec_model = Model::load_file(rec_model_path)?;

        let engine = OcrEngine::new(OcrEngineParams {
            detection_model: Some(detect_model),
            recognition_model: Some(rec_model),
            ..Default::default()
        })?;

        Ok(Self { engine })
    }
}

impl OcrProvider for LocalOcrProvider {
    fn extract_text(&self, image: &DynamicImage) -> Result<String> {
        let rgb_img = image.to_rgb8();
        let dims = rgb_img.dimensions();
        
        let source = ImageSource::from_bytes(
            rgb_img.as_raw(),
            dims
        ).map_err(|e| anyhow::anyhow!("Failed to create ImageSource: {}", e))?;

        let ocr_input = self.engine.prepare_input(source)?;
        
        // Use the high-level get_text method for simplicity as recommended in docs
        let text = self.engine.get_text(&ocr_input)?;
        Ok(text)
    }
}

pub struct MockOcrProvider {
    pub text: String,
}

impl OcrProvider for MockOcrProvider {
    fn extract_text(&self, _image: &DynamicImage) -> Result<String> {
        Ok(self.text.clone())
    }
}

pub struct CloudOcrProvider;

impl OcrProvider for CloudOcrProvider {
    fn extract_text(&self, _image: &DynamicImage) -> Result<String> {
        anyhow::bail!("CloudOcrProvider not implemented.")
    }
}
